The requested simulator is incomplete. No exact physical miner target passes
all required gates. Keep original, source-adapted and component evidence separate.
The user asked to continue. The latest status answer said it was incomplete.

All three delegated agents stopped with the account usage-limit error. The tool
reported a retry date of Oct25,2026 at08:48AM, without a timezone. Do not retry
the unchanged external failure. Their edits and test outputs remain in this
workspace. The root agent continued the running tests and fixes below.

Browser:

- Actual memory trace013 found eleven failed16GiB virtual reservations. Its
  independent review is in browser/run-013/memory-independent-review.json.
  The launcher now permits64GiB virtual space. Resident memory remains768MiB
  per browser leaf, guest RAM2GiB and outer address space3GiB.
- Probe014 timed out during active UI startup. Probe015 failed preparation on
  storage quota. Probe016 omitted heavy logs and allowed120s NewSession inside
  a150s startup budget; it still timed out. The parent and WebExtensions main
  threads were running. Both cgroups were cleaned, with no OOM or PID-limit hit.
- Renderer017 creates a real800x600 PNG. It passes rendering only. Inspection
  showed Pending, because the raw data URL contained an unescaped fragment
  marker inside its script. The source now percent-encodes the whole HTML.
  No script or native-dashboard pass was inferred from that screenshot.
- Probe018 failed from frozen browser/prepared-018. It tested the same real
  two-profile DOM/service path with only profile-scope add-ons enabled. This
  is a diagnostic profile policy, not a default change. Exact captured
  XPIProvider source is in browser/source-xpi-006 and documents scope behavior.
  Full run: /tmp/md-sim-browser-run-018. Its120s NewSession deadline expired
  with the parent UI thread running. No component pass. The reporter is frozen
  beside the driver.
- Probe019 failed before QEMU execution because the longer CPU budget did not
  match the confinement policy. A distinct720s support-browser VM policy fixes
  the mismatch; miner policies remain separate. Probe020 then ran176s and
  Firefox crashed after two content-process launch failures. No cgroup OOM or
  pids.max hit was recorded. The UID-wide RLIMIT_NPROC128 is a hypothesis for
  that failure, not yet a finding. The existing diagnostic process report
  captured the parent as a zombie and the crash helper still running.
- Probe020's embedded limit summary retained old numbers. Its frozen launcher
  actually used CPU300s, total startup300s and NewSession270s. The current
  summary now uses the launcher's constants. Historical evidence is unchanged.
- Probe021 completed its failed-syscall trace. It records actual clone3 and
  clone EAGAIN, followed by the Firefox crash. Leaf pids.peak is126, with two
  harness tasks outside that leaf under UID-wide RLIMIT_NPROC128. Cgroup PID
  and OOM hit counters are zero. Its complete9622-byte trace, exact source and
  CAS audit are in browser/run-021/root-evidence-audit.json.
- The launcher now allows512 tasks per UID and256 per browser leaf. Linux
  counts threads in these limits. Memory and browser sandbox policy remain.
  The actual private-VM group-check002 passes all migration/FD denials,
  detached-helper removal and empty-leaf checks with this policy. Its byte
  audit is browser/group-check-002/root-evidence-audit.json. This root audit
  is not a separate author review.
- Probe022 passes the complete browser component contract. The frozen reporter
  checks actual JavaScript, live service changes, two fresh profiles, cookie
  isolation, all input hashes and empty groups. The report is
  browser/run-022-report.json. Wall time was613222ms. The two browser startups
  took249249ms and243498ms. Peak tasks were184 and181. This explicit HTTP
  fixture is not a firmware or native dashboard pass.
- Native011 remains prepared only, with the older014 helper. It must not run
  or pass under a mismatched later source. The next combined freeze must use
  actual successful browser inputs and budgets. Native HTTP/RPC010 passed,
  but browser, whole-board reboot and mining gates remain open.
- Native012 failed before execution because base ELF capture flattened
  library aliases while the browser payload kept the same paths as symlinks.
  Preparation now reconciles such paths only when terminal bytes and modes
  match. Different bytes, modes, missing targets, cycles and escapes fail.
  Five VM tests and strict Clippy pass. Native013 passed preparation.
- Native013 ran237200ms and failed the200s native readiness deadline.
  Original Linux, ubus/procd/rpcd/uhttpd start; the initial Lua login probe
  had not returned. No browser started. Complete output is at
  /tmp/md-sim-native-network-013. Compact logs and decoded inner evidence
  are in network-vm/native-013. The larger combined system also needs more
  memory than browser-only2048MiB: root files916MiB, nested guest1GiB,
  browser leaf768MiB, plus both kernels and emulator. New combined limits
  are4096MiB guest RAM,6GiB outer virtual space and256MiB translation cache.
  Native helper now records timed process/memory observations during startup.
  The200s readiness deadline is unchanged. These changes are not yet executed.
- The combined VM
  permits1260CPU/1440wall seconds. The native service permits900CPU seconds;
  post-readiness budget is1200 seconds. The browser helper has780wall seconds.
  These support-tool budgets use the measured browser022 and native010 runs.
  Ordinary network deadlines and guest boot readiness did not change.
  Use the frozen reporter after completion. Native011 remains unexecuted.
- A review finding is fixed: the final native DOM observation now includes
  login state, and the verifier requires it to remain false. Eight verifier
  tests and strict Clippy pass. No new independent agent review is available.

ESP firmware and INA260:

- Backend016 built successfully at /tmp/md-sim-esp-qemu-build-016. Its
  assessment is a backend-build pass only. It includes the new INA260 model.
- INA qtest001 failed only after a clock step extended past the final pending
  timer. Isolated inspection of exact archived QEMU9 source shows its virtual
  clock stops when no timer remains. qtest-clock-inspection-001 preserves the
  exact source/hash. The test now advances exactly to the conversion deadline:
  2240000ns minus200ns remaining STOP time minus470200ns preceding read time.
  It retains all434 exact responses. INA qtest002 passes with unchanged model.
- Original probe023 found the INA device was instantiated only in the register
  diagnostic branch. The power/ASIC mode therefore still saw NACKs. The
  fixture device creation is now outside that branch. Probe024 executes the
  archived factory bytes and reads register2=0x0fa0 (declared5V input), and
  current/power zero (declared0A input). Nine bus events have zero model faults;
  continuous conversion reaches20633 before the run stops. No requested core
  voltage was used as a measured input.
- Probe024 retains the real ASIC UART query55aa520500000a. There is no ASIC
  response model yet. Firmware remains at BM1397 detection. Display, regulator,
  Wi-Fi and other board work remains. The QEMU process ended with SIGKILL under
  the bounded diagnostic; this is not a complete boot or mining pass.
- Frozen qtest drivers: bitaxe/ina260-drivers-002. Frozen corrected firmware
  probe: bitaxe/ina260-drivers-003. All37 peripheral Rust tests pass. Root moved
  its new test target directory out of model source to
  bitaxe/sar-test-target-root-001 to keep build output out of source capture.
- Original probe025 adds the actual BM1397 UART decoder. It retains the real
  outgoing query and a decoded broadcast READ of register0. It sends no reply.
  The codec handles bounded framing, checksum errors, fragmented input and
  reset. Its job conversion passes3072 comparisons against bitcoin_hashes
  hashing complete80-byte headers. Four regression tests pass. This is not
  fresh mining or an ASIC execution gate. Frozen probe sources are in
  bitaxe/bm1397-codec-drivers-001. Its source snapshot precedes an additional
  job-frame corruption test; the runtime functions are unchanged by that test.
- Exact isolated source inspection bitaxe/bm1397-source-002 verifies original
  bm1397.c, its packed header, crc.c/header, mining.c and utils.c. The existing
  loose BM1397 C references match those extracted source hashes. The current
  codec is crates/md-sim-firmware/src/bm1397.rs. ASIC state, responses, nonce
  scheduling, filter and Wi-Fi work remain.

S9:

- Latest backend031 is /tmp/md-sim-qemu-backend-031, SHA
  481c19228252b2d5103242700cfca251765db31eba894e279f98e672d7c219b9.
  ONFI qtest003, NAND timing007 and generic NAND005 pass their scoped checks.
- Actual spl-onfi-development-003 ran unchanged SPL, U-Boot, kernel and init
  with two CPUs. The prior NAND wait warning is absent in this run. It mounts
  SD extroot, starts ubus/procd and modules. The blank development NAND has no
  rootfs volume; the ECC-strength warning remains. BootROM entry adaptation,
  unresolved physical NAND identity and incomplete devices keep the original
  board gate open. The run ended at its120s deadline with untruncated traces.
- Read qemu-s9/quota-checkpoint-031.md together with these newer results. It was
  written before the successful timing007/generic005/boot003 runs. UBSAN of the
  latest timing change is now tested in the recovered backend032 below.
- Backend032-ubsan already existed from an earlier completed agent process.
  Root's attempted rebuild refused the existing output before any write.
  Do not rebuild it again without cause. Its binary SHA-256 is
  fbd0631bb8c104debbdc86e36f7e622a08cda342c121b210de7ac57d8d3865b1,
  size19157424. All five patch/model input identities match031. Full input
  directory: /tmp/md-sim-qemu-backend-032-ubsan.
- Fresh nand-timing-qtest008-ubsan and onfi-qtest004-ubsan pass with that
  backend and the frozen root qtest driver in qemu-s9/ubsan-drivers-032.
  Actual spl-onfi-development004-ubsan ran120s, reached unchanged SPL,
  U-Boot, Linux, both CPUs and procd/ubus/init. No runtime sanitizer error
  or nand_wait warning was observed. The same blank-NAND rootfs and physical
  identity gaps remain. stage-review-root.json rehashes the exact inputs,
  joins component evidence and checks both logs. Its SHA-256 is
  f9500a7fc14599c761af1f541418d2134459d6f174ea71b6c6c375a587b8c65a.
  This is a root audit, not a separate author review or a board gate.

Storage and live process ownership:

- Sandbox command startup can fail with quota122 near38GiB/tmp usage even when
  df reports free space. Cleanup002 removed6.38GB, and cleanup003 another12.59GB
  after full byte and SHA-256 comparisons. Restore maps are under browser/.
  The latter also covers old S9 backend019–030 CAS inputs retained in031.
  Keep captured-003's browser CAS and backend031's CAS. No original archive or
  unique evidence was removed. Historical runs need copies restored before
  replay. Do not use hardlinks.
- browser/quota-recovery-003.md records deletion of one verified duplicate
  raw result through the file editor to recover shell startup. Its compact
  byte-identical copy remains in browser/run-013/result.json.
- Cleanup004 removes2746408491 bytes of verified duplicate browser016–018 CAS
  objects. Cleanup005 removes2658047302 bytes from browser019–020 duplicate
  CAS and combined initramfs files. Both have complete restore maps under
  browser/. Captured003 and each retained same-run combined-image CAS object
  must remain. Compact records016–020 retain exact logs and manifests.
- Duplicate-payload-cleanup006 removes2051925235 bytes from five historical
  browser payload files. Qemu-s9 duplicate-overlay-cleanup004 removes
  1011141072 bytes from eight historical backend overlays, retained in031.
  These restore maps supplement earlier maps. Do not remove their keep paths.
- Root owns an idle shell session70212 for emergency cleanup. Close it when
  no longer needed. Do not kill unrelated processes. No integration run is
  pending at this checkpoint. Native014 is the next combined freeze and run.
- Browser duplicate-cas-cleanup007 removes3478823609 bytes in7010 objects
  from runs021/022 after byte and SHA-256 comparison. Its restore map also
  covers duplicate combined images and payloads. Compact logs remain under
  browser/run-021 and browser/run-022. Keep all map destinations.

Workspace tests passed118 tests in52 suites. Strict all-target Clippy passes
after two new test-only clone lints were fixed. Affected library and native
verifier tests and release builds pass after the native budget change.
Logs are in checks-2026-10-20-002. The full frozen matrix with
--require-complete, all exact-target
fresh mining/Core acceptance, original-board gate and release review remain.


Root continuation after native014 and ASIC identification:

- Native014 used4GiB guest RAM and the actual archived LuCI service. The
  service became ready at141785ms. HTTP/RPC, SSH and TLS checks ran before
  Firefox. Firefox started in72930ms. Its first SSH page navigation reached
  the30s page-load deadline. The browser gate failed. No assertion was
  weakened. The full run is /tmp/md-sim-native-network-014; compact logs are
  network-vm/native-014. Selected run is
  1dd6323267dbd53f055c4b25ce3eefc2ae9920192ff1479771d885127c491585.
- New diagnostics check the browser helper's own SSH tunnel before Firefox,
  retain its HTTP status and body identity, and capture DOM/navigation state
  on error. Native015 is running from native-driver-015. Frozen driver SHA
  9495c2478b3ff894107a41061571450313ac855d02eefd21e31a86a4356555cb.
  It keeps the same deadlines as014. Do not infer a pass before its report.
- Full latest workspace tests passed122 tests in52 suites, zero failed or
  ignored. See checks-2026-10-20-002/workspace-latest-tests.log.
- Native012/013 duplicate cleanup008 removed1599694692 bytes in3656 objects.
  Native014 cleanup009 removed1891622442 bytes in3551 objects. Restore maps
  are under browser/. Keep captured003 CAS, native010 CAS, and all other
  destinations named by the maps. No archive bytes or unique logs were removed.
- Rust incremental build cache cleanup freed3993103471 bytes. The cache can
  be rebuilt. See checks-2026-10-20-002/incremental-cache-cleanup.json for the
  exact measured count. Use CARGO_INCREMENTAL=0 while space is limited.
- Original Bitaxe probe026 ran unchanged factory image3fd36298681d7797429dd72610041d731558c5253430902a3c7feeced232557c.
  The guest sent55aa520500000a. The stage model computed replyaa5513971800000006.
  The actual guest logged chip0, core field0x18 and address0. It then reached
  the45CPU-second bound without another command. The next step is a private
  monitor/trace diagnostic of this stall; do not increase the bound as a fix.
  Root stage audit SHA3fd478fd746e73902091c7085d2d541c3a94d60de82d7f811ef641eae746a58b.
  This is identification only. GPIO power/reset bindings, full ASIC state,
  fresh mining and original-board gates remain incomplete.
- All three separate agents remain stopped at an account usage limit. Root
  continues implementation. New changes still need an independent review.


Root continuation: ADC diagnosis and native015.

- Native015 failed after470439ms, with untruncated logs. Its own SSH tunnel
  preflight returned the real LuCI login form in7978ms. Firefox started in
  71382ms. The first page navigation expired at30s. A content-process actor
  appeared32s after navigation began. The following5s DOM diagnostic also
  expired. This is a failed browser run, not a transport failure or a pass.
  Compact evidence: network-vm/native-015. Selected run:
  71973806c8d8670cd03cf5c0a91f580b87e1f3af9b9ce97c7d4b1e621aee131d.
- Native016 is now running from native-driver-016. Browser helper hash
  df682ea8e37e5ecf428a8e2ed6b955862b8bbd058b0a2e79181e4c560893de67.
  It explicitly records90s page-load and105s command budgets for the browser
  under TCG. The ordinary guest/network timers and all DOM, asset, live-poll,
  credential, cookie and cleanup assertions remain. The component fixture
  keeps its30s page-load default. Scoped tests and Clippy pass.
- Probe027 read both CPU registers, but its monitor $pc expression was
  unsupported. Probe028 uses the observed code addresses and proves CPU0
  is polling SAR DONE at0x6000880c, PC0x420e0839. This supersedes the UART
  timeout hypothesis. The external ADC1/channel1 input is disconnected.
  Exact archived adc.c (hashbbee8c62e7442de3bfb71008481c65850e6a846c8520c2eb1b96c8a149db80d7)
  confirms that this is the firmware's Vcore channel. See adc-source-002 and
  original-probe-028/stage-diagnosis-root.json. Original firmware is unchanged.
- Generic SAR binding now has20 explicit initial voltage properties. Default
  disconnected inputs still do not return a valid sample. This adds a real
  voltage-input interface, not a regulator connection. All37 Rust peripheral
  tests, scoped Clippy, and the extended C ABI check pass. The C check uses
  AddressSanitizer/UBSAN with LeakSanitizer disabled because host ptrace is
  denied. Backend017 is building in /tmp/md-sim-esp-qemu-build-017, with frozen
  drivers, model source and static library in bitaxe/sar-input-drivers-001.
  Next: execute its literal qtest vectors, then the explicit0.55V ADC stage
  probe. Its input must not be called measured physical ASIC voltage.
- ESP duplicate cleanup002 removed2674351812 bytes from160 old010–014 CAS
  objects, retained byte-identically in016. Browser cleanup010 removed
  1772691647 bytes from3505 native015 objects and image copies. Full restore
  maps are retained. Unique firmware, source, backend binaries and logs stay.


- Backend017 completed. Binary SHA-256 is
  ce946687f1b97577ba2a6434d08b38a8a6dfeb8cbf072bfbf2545d2e7356cd9e,
  size17058432. Its47 literal qtest responses pass in sar-input-qtest-001.
  Root rehashed73 mounted inputs across the build, qtest and original probe.
  See bitaxe/esp-qemu-build-017/stage-review-root.json and full/tmp build017.
- Probe029 reached Chain Inactive after chip identification. Its live QEMU
  stderr split one device JSON record. Probe030 fixes the diagnostic stream
  separation and retains QEMU logs even when the model reports an error.
  All eight device JSON records parse. Inner exit1 explicitly reports the
  unsupported command; all board/mining gates remain false. Driver freeze
  bm1397-identify-drivers-004 has probe hash
  95ca010fa3190e0dec0e858207ada2fd6ae91f86498772cdeaaae365551958b9.
  Its original image and explicit0.55V ADC stage input are unchanged.
- Nine BM1397 tests now pass. The new ticket-filter component preserves
  programmed bits and the fixed32-bit requirement. It uses the shared engine
  Filter probability/intersection implementation, without a hidden override.
  Actual ASIC initialization, address state, core registers, PLLs, clocks,
  work queue and programmed-filter mining remain to implement and validate.

Native browser016–018 and BM1397 address work:

- Native016 completed navigation but real uhttpd returned HTTP502 after its
  60s CGI deadline. The DOM assertion correctly failed. The browser budget
  was90s page/105s command. The firmware timeout was unchanged. Compact
  evidence is network-vm/native-016, including browser-failure-dom.json.
- Native017 held QEMU stdin and captured bounded service/PID observations.
  It failed an authenticated HTTP request after20s with7623 bytes received,
  before Firefox. Wall time238349ms. LuCI was running in the recorded sample;
  no blocked random read was observed. Compact evidence is network-vm/native-017.
- Native018 is running from network-vm/native-driver-018 in
  /tmp/md-sim-native-network-018. The support VM now has two virtual CPUs with
  multithreaded TCG. The guest firmware and all HTTP/CGI timeouts stay unchanged.
  Repeated full process lists were removed from the diagnostic sampler.
  Formatting, scoped strict Clippy and release builds pass. This is a resource
  contention hypothesis, not a proven fix. Root tool session53071 owns the run.
- Native017 duplicate cleanup012 retained byte/hash verified copies and
  compact logs. Restore map: browser/duplicate-cas-cleanup-012/restore.jsonl.
  It removed3505 duplicate objects/files,1772690030 bytes.
- Exact LuCI dispatcher/http source extraction is retained in
  native-s9/dispatcher-source-001 and native-s9/luci-http-source-001. Each used
  the isolated7z closure and rechecked all three original FIT components.
- BM1397 address assignment now has explicit per-chip state. It follows the
  archived ESP-Miner/Gekko INACTIVE then sequential SETADDRESS sequence, including
  logical address0. Register0 includes the assigned address. Reset and power
  loss clear assignments. Unknown register writes remain unsupported. This
  code is under test; no mining or physical board pass is claimed.

Native020 and shared midstate engine checkpoint:

- Native018 ran551587ms. Native service readiness136471ms. Real Firefox
  submitted login and received the HTTP200 overview. It failed while the
  redirected document was still loading; body/uptime were absent. The source
  template confirms id=uptime and actual5s XHR polling. The driver now applies
  its existing90s page allowance to login redirects, then30s for telemetry.
  Compact evidence: network-vm/native-018.
- Native019 ran330255ms and failed before Firefox: nginx returned502 during
  native uhttpd restart. The fixed3s sleep raced the delayed restart. New
  native_restart.rs retains at most8 probes in60s; only502 is retryable, old
  live PIDs cause another probe, and the final new PID must match a subsequent
  normal RPC check. Configuration readback/restoration assertions remain.
- Root review found the browser required HttpOnly despite original LuCI
  dispatcher.lua and actual HTTP headers having neither HttpOnly nor Secure.
  The existing separate service verifier already checks the original exact
  Set-Cookie form. Browser checks now require the original flags plus exact
  path/domain and one sysauth cookie. Separate-profile isolation still applies.
  A stale unit fixture failed once; it was corrected, not excluded.
- Native020 is frozen at network-vm/native-driver-020. Its source/binaries
  include these fixes and remove the temporary periodic guest process sampler.
  It uses /tmp/md-sim-native-network-020. No firmware, HTTP or CGI timeout was
  changed. Run its frozen reporter after completion.
- Shared Engine now accepts bounded MidstateWork alongside full-header work.
  Both use one queue, round-robin scheduling, one aggregate limiter, cancellation
  and reset. It returns a midstate index without inventing a header version.
  BM1397 Job::engine_work preserves the programmed ticket filter and its fixed
  32-bit condition. It is not yet connected to a complete ASIC firmware path.
  Fresh engine tests compare each digest with independent complete-header
  SHA-256d. A fixed genesis regression checks the scheduled BM1397 filter and
  response bytes. These do not count as fresh firmware mining or Core gates.
- Full workspace after midstate/cookie changes:129 tests in53 test binaries,
  zero failed/ignored. Latest scoped native restart suite:9 tests pass. Strict
  Clippy and release builds pass. Logs are in checks-2026-10-20-002.
- Existing Rust library candidates found: GPTechinno/bm1397-protocol0.2.0 and
  GPTechinno/bm13xx-rs. No dependency was added. static.crates.io acquisition
  failed DNS; web source-file retrieval also failed. Exact bytes/revision remain
  unresolved, so no compatibility or source identity is inferred.
- Cleanup013 and014 retain verified duplicate-copy restore maps for native018
  and019. They remove1772690510 and1772690363 bytes respectively.
- New browser/concatenated-image-cleanup-001/restore.jsonl records7 combined
  initramfs CAS objects,3156113649 bytes, as exact concatenations of retained
  base gzip plus common payload gzip. All bytes and hashes were compared.
  Restore these CAS objects by concatenating parts in order BEFORE applying
  earlier duplicate-copy restore maps. Keep each native013–019 initramfs.cpio.gz
  base and /tmp/md-sim-native-network-013/payload.cpio.gz. No unique information
  or original archive file was removed.

Native020, original ESP032 and S9IO packet checkpoint:

- Native020 is still running, owned by root tool session84353. Frozen inputs
  and binaries remain in network-vm/native-driver-020; run output is
  /tmp/md-sim-native-network-020. Do not start a duplicate test.
- Original ESP032 finished in2874ms, inner exit1, no trace truncation. It
  identified the chip and executed INACTIVE/SETADDRESS0, then stopped at
  unsupported register0x80 value0. stage-audit-root.json retains exact hashes.
  It is an implementation-agent review, not an independent review or board pass.
- Verified archived Braiins HDL, driver and test-bench source in
  braiins-fpga-source-002, with exact member hashes in members.json. The reader
  is frozen in braiins-fpga-reader-002. No acquired code was executed on host.
- New s9io packet component matches literal archived transmit/receive vectors,
  extended IDs, CRC failures and stream recovery. Four tests passed in
  checks-2026-10-20-002/s9io-packets-001.log. This is not a complete FPGA,
  firmware mining or target gate. UART timing, register/FIFO state and binding
  remain to implement. The HDL outputs VERSION0x00900200, while the SVD and
  version test expect0x00090002. The SVD also has BM139X control absent in
  this HDL. The HDL command receive path gates writes on work FIFO fullness.
  Preserve these conflicts; do not infer exact binary/bitstream compatibility.

Native020 passed (20 October 2026):

- Frozen driver: network-vm/native-driver-020. Actual output directory:
  /tmp/md-sim-native-network-020. Root tool session84353 completed.
- Frozen strict reporter passed. Compact evidence is network-vm/native-020.
  Selected run ID is ca1e04918cc477ed504ad704f12a5b48c800f2257fbbad0631585004cc9b117e.
  This is the execution identity, not the report file hash.
- Report file SHA-256 is
  4d56f489770a035f67ebe5c5303e1f2cf81d235f7e12163ae598b3859c1cecbb,2377487 bytes.
  Whole VM wall time776124ms; no trace truncation.
- Real Firefox login and page JavaScript passed on both the SSH tunnel and
  mutually authenticated TLS endpoint. Actual uptime changed from8m51s to9m15s
  over SSH and11m21s to11m40s over TLS. Archived CSS bytes matched.
- A fresh second profile had no inherited same-origin cookie. Both browser
  process groups were empty after cleanup; no OOM or process-limit events.
- Planned uhttpd restart retained one502 response at2865ms, then the actual
  new PID815 at15903ms. Configuration persistence across service restart and
  exact restoration passed. The guest HTTP/CGI timeouts stayed unchanged.
- Topology: separate private worker/client namespaces in one local Linux VM.
  This was not an external physical host. Guest environment remains adapted.
  Full guest reboot persistence, original-board acceptance, mining, full target
  matrix and independent review are incomplete. No exact target pass is added.

S9IO and storage follow-up:

- Full workspace after S9IO/native020:134 tests in54 test binaries,0 failed
  or ignored. Formatting, strict workspace Clippy and guide links pass.
- Cleanup015 removed3505 byte/hash-verified duplicate files from native020,
 1772717774 bytes. Restore map is browser/duplicate-cas-cleanup-015/restore.jsonl.
  It retains the unique combined-image CAS object and native020 base gzip.
- Storage probe001 executed the original kernel and confirmed ext2/ext3/ext4,
  VFAT and SDHCI support. No SD card was attached in that read-only probe.
- New storage-cycle mode uses captured mke2fs in the closed namespace to
  prepare8MiB ext2 storage. Two separate original-kernel boots share that disk.
  It writes a fresh hostname with UCI, then checks it and restores original
  system configuration. This is an adapted, orderly cold-restart component
  check; not warm-reset, power-loss or physical NAND acceptance.
- storage-cycle-driver-001 frozen binary hash is
  87d4c47f29e05eb318433aeca811bc65378b8a9fdbcfd55808e83e2b851e0a4b.
  Root session42197 owns storage-cycle-001. It is under test.
  The serial driver now retains stdin until QEMU is stopped. New SD images
  are exported losslessly as bounded gzip with raw and compressed hashes.

Storage-cycle002 and independent image verification:

- storage-cycle001 failed. It mounted the SD disk, then tried absent
  /etc/config/system. set-e exited PID1 and caused a real kernel panic.
  Inner exit1 after75828ms; no trace truncation. Preserve this failure.
- storage-cycle002 uses the actual uhttpd configuration and runs each script
  in a subshell so a failed assertion does not kill PID1. Its frozen driver
  is3c098e0c6f67544104388ffc0fcec1b317ea59da831829babf213cecdd09d1f1.
  Two separate original-kernel boots passed in15077ms. First UCI set357;
  second UCI read357 and restored100. Both completed with status0.
- All three8MiB disk stages are retained losslessly in gzip. Their raw and
  compressed identities are storage-cycle-002/storage-images.json.
- Independent ext2 reader report002 passed. It runs captured7z in another
  closed namespace, checks every saved process input, exact firmware hashes,
  fresh stdin marker and result/trace binding, and compares the restored
  configuration with the original archived ramdisk member. Report SHA-256:
  433cdfd569adeeebedb09784f560f018b5db1b0df7f342f3fed7b113b8ba5216,19653B.
  Reporter002 binary c71171a7a8cee408b496c29c791dd55bd7193ed00712d3e8a169a4ec2af51dd6.
  The failed001 run was rejected into storage-report-rejected-001.
- All current storage sessions have finished. Native020 has also finished.
  No firmware or browser integration process is still running.
- Root review found native020 original_config_sha256 names the SHA of the
  base64 RPC string (b4c946a65be037ef0d02ad6abf8588bcf0db8646034e63c29a8bef087a6e370d,5048B).
  The decoded file is302128daf55df98b268cc7296326a8fd33786747da9b3ca58762730aa675b077,3784B,
  identical to the independent archive extraction. Exact restoration remains
  valid, but future native report fields need explicit encoding/raw identity.
  Audit: network-vm/native-020/config-encoding-audit-root.json.
- Next dashboard work: connect a run-owned SD overlay to the selected native
  network run, test full guest restart through that SAME published endpoint,
  renew guest authentication, verify committed config, restore it, and retest
  real Firefox/SSH/TLS. Do not substitute the separate storage component pass.
- S9IO is still packet conversion only. AXI/FIFOs/UART timing/binding and
  actual ASIC mining remain work. The extracted HDL/source compatibility with
  each selected firmware/bitstream must be established before execution.

Final checks for this storage increment (task still incomplete):

- Full workspace after storage:134 passing tests in55 test binaries. No
  failed or ignored tests. Native config-encoding field correction passed
  all9 scoped remote_namespace_check tests. Native020's immutable report
  still uses its original field name. Future reports now explicitly use
  original_config_rpc_encoding=base64 and original_config_rpc_sha256.
  Retest those future reporters with the forthcoming selected-run restart.
- Formatting, strict Clippy and guide links passed before this small metadata
  correction; the scoped test passed after it. Release workspace build is
  now running. Check its tool session/log before another Cargo invocation.

- Release workspace build completed successfully (session59047), log
  checks-2026-10-20-002/workspace-release-after-storage.log. No Cargo jobs
  remain. Last guide-link check passed. New files for this increment:
  crates/md-sim-firmware/src/s9io.rs, tests/s9io_packets.rs,
  tests/fixtures/s9io-reference.json, examples/support/s9_storage.rs,
  examples/s9_storage_report.rs.
- Native002 storage verifier proves raw original config hash302128da...b077.
  Native020 API's b4c946a6...370d hash is its base64 encoding, not different
  file content. That conflict is resolved with the encoding audit. Current
  source explicitly labels the RPC hash; current source needs a new native
  integration run as part of adding published-run restart persistence.
- Storage implementation next step: first exercise a real guest warm reboot
  using the same run-owned disk and original kernel, then integrate the SD
  fixture and reboot through native_network's selected SSH/TLS endpoint.
  Its current helper has -no-reboot and sends startup only once. Remove that
  only in the explicit persistence mode, observe real reset/boot output,
  re-send the declared startup script, renew native authentication, and
  verify config through the original endpoint. Preserve run identity across
  the restart. Capture compressed disk bytes within explicit trace limits.
- Resource note: /tmp reports36GiB used (quota near38GiB), root about1.5GiB
  free. Before another large browser VM, perform only verified duplicate
  cleanup with restore records. No originals or unique evidence may be lost.

Warm-reset implementation checkpoint:

- SerialGuest owns one emulator, retains stdin, and can run another script
  after a real /sbin/reboot -f. Prompt matching discards any prompt before
  the actual kernel reset line. Fragmented-stream regression tests pass.
- storage-cycle-driver-003 cold test003 passed and independent report003
  passed. Warm test001 did execute the real reset and retained990, but its
  strict report failed because the diagnostic record followed the shell
  prompt on the same line. Do not mark that report as a pass.
- Driver004 adds explicit newline boundaries to image and warm-reset records.
  Frozen driver38d3b9dd57632dd415680827d3cfbd10ca4d05b8e356b70b035f901646e14626,
  reporter16983d254d4cd67e7acd3968f9e725c3d2a2902e52e52e3b855f3c97b36eefb4.
- Root sessions88615 (cold storage-cycle-004) and8501 (storage-warm-cycle-002)
  completed successfully. Both frozen004 reporters also passed, into
  storage-report-004 and storage-warm-report-002 directories. They checked
  actual reset order, one emulator PID, exact input hashes, disk contents
  through independent7z, fresh marker, and archived configuration restoration.
- No published native-network reboot test has started. Native020 browser
  remains the only completed combined native SSH/TLS/Firefox run.

Storage004 completion and space recovery:

- Cold004 retained fresh226 and restored100; report SHA-256
  d07d93ca84d0d10218031a406cca954f263400d3243c719f1d93a30c94c86885.
  Warm002 retained fresh109 and restored100; report SHA-256
  0e4131651cf84e1334977bdbf8d526f63b1ea9ae3096431f55fdf65b10036a5a.
  Both independently compare the restored file to archived ramdisk bytes.
- browser/concatenated-image-cleanup-002/restore.jsonl records9 removed
  combined images,3719518098B. Restore by concatenating its exact parts.
  Keep browser004/009/013 payloads and004/009/013/016/017/018 base images.
- native-s9/storage-duplicate-cleanup-001/restore.jsonl records386 duplicate
  CAS files,601080584B. Copy retained source paths to restore. Do not hardlink.
  Every removal followed byte comparison, hash verification and fsync of
  the restore journal. No original archive or unique evidence was removed.
- README now links both verified004 restart commands and reports.
  Published native reboot persistence is still the next implementation step.

Published native reboot test021 is running:

- Root session99908 runs frozen native-driver-021/vm-driver, output
  /tmp/md-sim-native-network-021. Do not start another large VM until this
  ends and space is checked. Reporter0024eb619d8daa9b441287b838486348a78867dacc2d299968261a2788cec7a5,
  helper9cd0fcbef62ccbe05df227c70a78e3170ee73f3f6a9e2f5227c0723aa8d6765d,
  VM3ad17a3a68753eb24c6c9dfee5aad323ed27d390ee301c13aa84330843310d66.
- New shared s9_sd_image.rs formats/captures an8MiB ext2 SD inside isolation.
  The native manifest binds its initial hash and captured mke2fs. The startup
  script mounts the same SD /etc on each boot. The helper observes the real
  kernel reset before it sends startup again. It retains one emulator PID,
  serial/startup hashes, initial/final disk images and both readiness events.
- New native_reboot.rs writes a run-derived fresh value through actual RPC,
  requests sync/reboot through the same TLS endpoint, requires outage and
  recovery, compares kernel boot IDs, reads committed config and restores
  exact prior bytes. Existing service restart checks remain. Firefox then
  runs against the same selected endpoint. native_storage.rs independently
  checks retained image/trace identity and reset order. Not passed yet.
- Firmware timeouts remain unchanged. Outer evidence trace now8MiB for
  nested runs, with explicit bounded host readers, to retain disk snapshots.
- Scoped tests and release build passed before this frozen integration.
  Full current workspace checks remain due. Independent agents remain
  unavailable because their recorded account usage limit has not changed.

Native021 failed; native022 is now running:

- Session99908 ended. Native021 ran461581ms, emulator exit0, no trace
  truncation, but its client failed on curl28/20s timeout during actual
  guest reset. The firmware had committed fresh412 and returned reboot RPC
  success. Serial captured the actual kernel restart; helper observed it at
 402836ms in the SAME emulator PID7. No second boot was checked.
  Compact failure files: network-vm/native-021. It is NOT a pass.
- Fix: only the bounded reboot readiness probe can record curl28 as a
  transport timeout and continue. Normal HTTP/RPC requests retain strict
  failures. Readiness still needs native403, same published run, new kernel
  boot ID, preserved config and exact restoration. Other transport errors
  fail. Do not count timeout itself as HTTP status or readiness.
- Review also found failed runs lost their SD artifacts at VM shutdown.
  The orchestrator now retains original formatted SD bytes and a separately
  labelled failed-run snapshot. It makes no consistency claim for that
  snapshot. sd capture hashes and compresses one bounded read of the bytes.
  Native021 cannot retroactively gain those lost artifacts.
- isolated_network_vm now uses existing live trace capture for guest.log and
  backend.log. It retains output during execution, with no raw control port.
- Current session24315 runs frozen native-driver-022/vm-driver into
  /tmp/md-sim-native-network-022. Reporterb21f5fdca95723015253173c8fb2738721ec92063dc5f14427f4421aa4c6a382,
  helperb87935a994cceac7bbe35ebe19a54f45ecb54ebd61271f63f5d488f0331db471,
  VM9d8016e33f19440775b4fd1fdbc86c5400b81a93ebd3088aa3f0a2d743bdcdcf.
  Use its frozen reporter after completion. No result yet.
- Native021 duplicate cleanup016 reclaimed1772937129B from3505 files.
  Restore map browser/duplicate-cas-cleanup-016/restore.jsonl. Preserve its
  source paths and remaining unique combined image. No archive change.

S9IO receive queues and clean checks:

- Added s9io/receive.rs functional receive FIFOs with256 command words and
 1024 work words, two-phase reply writes, pending-response backpressure,
  IRQ read/clear behavior and separate reset/enable behavior. Preserve HDL
  defect: work FIFO fullness gates command writes; full command FIFO drops
  writes without generating a CRC error. Extended IDs use transmit state at
  the SECOND write, after any stall. Seven source-vector/behavior tests pass.
  Source hashes checked again in s9io-receive-review-001.json. UART/CRC/AXI
  timing, TX/MMIO integration and exact bitstream identity remain open.
- Empty Cargo cache clean release workspace check passed138 tests in55 test
  binaries, none failed or ignored: clean-build-2026-10-20-003/tests.log.
  This preceded the small reboot-timeout/failure-snapshot/live-trace fixes.
  Scoped Clippy/tests/release builds passed after those changes (003/005 logs).
- Public search found dvb-WarpAxe Rust simulation references. Source page
  fetch was unavailable and host curl failed DNS. No source identity or
  executable was acquired. warpaxe-discovery-001 records the exact blocker;
  do not retry unchanged DNS failure or claim reuse/compatibility.

Native022 failure, account fix and native023 execution:

- Native022 completed908894ms, outer emulator exit0 with no trace truncation.
  The scenario FAILED. The frozen reporter rejects it. Compact evidence is
  network-vm/native-022, including raw HTTP, serial/startup logs and both
  initial and failed-run SD snapshots. Snapshot hashes were independently
  checked after decompression. No orderly-shutdown consistency claim.
- Real archived services completed the reboot sequence: same selected run
 79353e31ce2796e96ee49379c3cd4bbb51501eb97292af2a34c67134d0179ed6,
  kernel boot ID changed, fresh value566 survived, original config restored.
  Same QEMU PID7: firstready147689ms, actualreset409297ms,
  secondready580786ms. This partial observation is NOT an overall pass.
- Firefox then reached190 tasks; nested QEMU aborted with signal6 and
  qemu_thread_create EAGAIN. Both had UID1000. Linux RLIMIT_NPROC counts tasks
  by user across PID/network namespaces. Worker cap128 was consumed by the
  browser despite browser cgroup256/UID512 limits. No browser OOM/pids event.
- Fix uses browser UID/GID1001, worker stays1000. Shared browser ownership,
  private passwd/group, cgroup delegation, diagnostics and adversarial group
  helper now agree. Native helper checks distinct accounts; browser report
  verifier rejects UID/GID1000. All resource bounds are unchanged.
- Strict all-target Clippy, nine remote report tests and release helper builds
  passed: checks-2026-10-20-002/native-account-{clippy,tests,build}-001.log.
  Complete integration running in session22886, /tmp/md-sim-native-network-023.
  Frozen native-driver-023 reporter87cbce617339e96cf90addab5b1e4aeb2799d885a4123a1a9fc7878b3b201897,
  browser9f159fcd17c59a98da0c6356ca947d4fb85f879219afbadc1f1f6e6e01a53086,
  helper7155ed849067d318071809b037f7c89ebaa632288e2fae0647aa928e2a665edf,
  VM0b4e8cca1fdc6491197325fa70c79c1d4119c18a1b7bdf60fbcf48e66f9586e4.
  Use its frozen reporter after completion. No result yet.
- Native022 cleanup017 compares bytes/hashes before removal and records
  restore paths. Preserve native022 unique combined CAS, native013 payload,
  browser captured003 CAS and native010 CAS. Archive unchanged.

Other completed checks before account fix:

- New shared SD formatter/capture driver005 warm-cycle003 passed14463ms,
  fresh758 survived actual same-process kernel reset and restored100.
  Strict storage-warm-report003 passed; reportSHA
 562488f6f3bcbbbc770ef050b3000d4950f0b521da2a92d3479d6201e095a756.
- BM1397 clock-order storage tests13passed. Original frozen probe007 run033
  executes unchanged Bitaxe factory firmware: Read0,INACTIVE,SETADDRESS0,
  Write80=0,Write84=0, then visible unsupported Write20=1. Innerexit1,2912ms,
  no truncation; outer diagnostic wrapperexit0 is NOT firmware success.
  Exact identity/limitations in bitaxe/original-probe-033/stage-audit-root.json.
- Workspace139 tests in55 binaries passed, strict all-target Clippy/fmt pass
  after clock storage; logs workspace-{tests,clippy}-after-clock-storage.log.
- README links334passed after storage/S9IO/BM descriptions. Later changes
  need their own checks. All exact hardware completion gates remain open.
- Independent agents remain unavailable at the unchanged account usage limit;
  no independent review pass. No final response or reduced-scope completion.

Native023 outer failure and active native024:

- Native023 ended1030269ms with wrapperexit1. QEMU received SIGKILL.
  CPU usage was not saved, so CPU exhaustion is plausible, NOT proven.
  Old cap1260CPU seconds,2vCPUs,1440s wall. Host cgroup metrics not visible.
  Nested files were lost with the VM. Compact failure: network-vm/native-023.
- New outer wrapper retains child PID/exit/signal/wall/CPU/peakRSS after wait.
  New atomic client stage files are relayed as diagnostic-only
  MD_SIM_NAMESPACE_PROGRESS lines (HTTP/RPC, reboot, browser, SSH).
  These are not acceptance evidence. CPU allowance2880s permits2vCPUs within
  existing1440s wall bound. Guest clocks, browser/native deadlines, memory,
  and assertions unchanged. Current session62263 runs native024 in
  /tmp/md-sim-native-network-024. Use frozen native-driver-024/reporter.
  No result yet; last observed267s, live guest.log466B.
- Frozen024 reporter d12d28f2db8f2e0e90b96f99ac449040a22e7223d6ab7c28f8e675e71935e283,
  helper e4f78392e897a7001bb07ea99dcc3c80469b7b74bb8626241a6403fdf72d6c6c,
  VM1142faf6ceac437afd4c2a29baddc33688aa8d8bbad2bd9916fb0c157813789f,
  browserdd2aa58b7f3b039ad2b7688a4b13b508e53a00d407035be059487fe10b7322b1.
  Snapshot includes unrelated S9IO modules, which this run does not test.
- Native023 cleanup018 reclaimed1773001782B from3505 verified duplicates.
  Subsequent /tmp quota hit prevented exec startup. Emergency shell70212
  remains open. It removed verified concatenations022/023 via journal
  browser/concatenated-image-cleanup-003/restore.jsonl, reclaiming902376809B.
  Preserve base initramfs022/023 and native013 payload. Restore these CAS
  concatenations BEFORE copy maps017/018. Archive untouched. Exec works again.
  /tmp now35GiB, practical quota near38GiB despite df48GiB. Check before runs.
- Browser group cleanup003 passed4724ms under new UID1001, including denied
  sibling migration, protected descriptors and detached-child removal.
  browser/group-check-003/root-evidence-audit.json. Not a dashboard gate.

S9IO transmit/register/UART work:

- transmit.rs implements source-state TX sequencing,1024command/2048work
  words, work priority, partial-command stalls, source24-bit timer and
  LAST_WORK_ID on transmitted ID byte. Preserve full-queue pointer-wrap IRQ
  defect. UART/CRC readiness is still supplied by caller; simultaneous CPU
  strobes need enclosing model. Eight tests pass:22literal source vectors
  with stalls,5full TX/RX extended-ID vectors, independent two-midstateCRC08cc,
  capacities, reset/sync and timing. s9io-transmit-review-001.json.
- registers.rs implements source MMIO offsets, byte strobes/FIFO lane zeroing,
  IRQ masks, FIFO read effects, error-clear pulse and sync-ack reset requests.
  Empty read retains prior output across IP reset; never-read FIFO returns
  UninitializedRead. LiteralHDL00900200 version retained despite SVD00090002.
  Explicit buildID input, no compatibility override. Five tests pass;
  s9io-registers-review-001.json. Not AXI handshake timing or full FPGA.
- Workspace152tests/57binaries passed after TX/register changes,0failed/ignored.
  Strict Clippy and README commands passed; guide links335passed. These
  preceded UART/resource-progress changes. Latest scoped usage tests9passed,
  release helper build and strict workspace Clippy passed.
- Isolated source inspector now captures mod_divider.vhd from original
  dc5f5872... archive. braiins-fpga-source-003 has actual spec/result and
  new-member.json. Member SHA
  df3899461bfd9fad91fe6cc2d9e4b3a95fdd68d39d4b8c5cb1ed0a6428f7aa7c.
  All25 prior retained members were checked against their manifest hashes.
- uart.rs implements8N1/16samples,16-byteFIFOs,3-stageRx synchronization,
  registered12-bit divisor, false starts, bad stops, sticky overrun,
  full-before-concurrent-read behavior, FIFOclear versusIPreset, and divisor
  update only after serializer AND queue drain. Five tests pass, including
  every byte at divisors0/1/13, one at4095, independently constructed bit
  waveforms/exactTXperiods and fault/reset behavior. s9io-uart-tests-001.log.
  Strict workspace Clippy after UART passed (s9io-uart-clippy-001.log).
- UART not yet connected to Registers/TX/RX or QEMU. Serial CRC readiness and
  receive-state clock delays remain next dependencies. No HDL execution or
  physical calibration claim. Full latest workspace tests, UART source review
  and docs still due. All exact hardware/mining/full-matrix gates remain open.

Native024 complete adapted service result and connected S9IO component:

- Frozen024 driver completed1105202ms,exit0,no truncation. Frozen reporter
  d12d28f2...e283 passed. Selected native run
  70c6d7580b47a5e06f2067cf24e993ff010fd48703eaf199cd10fefb674329c6,
  identity5242B. Full reportSHA
  a8da068781ff8a45ae753c9064131acd4df35ad1513d4ae1982ee3a653b02df9,
  4113242B. Compact evidence saved network-vm/native-024;311 raw HTTP files,
  full namespace/browser/service reports, original logs and two SD captures.
- Same nestedQEMUPID7,one observed kernel reset,two READY records. BootID
  b9f3f0e1-eea1-4bd5-91cb-b02cf45e99c0 ->
  b7472407-1839-406c-9ba4-65067594ad71. Fresh549 survives reboot; exact
  original config restored100. Both8MiB SD gzip/raw identities independently
  checked during retention. Lifecycle serial55336B/startup137227B retained.
- Firefox actual SSH/TLS DOM login/assets/page polling passed. BrowserUID1001,
  workerUID1000; fresh same-origin profile has no inherited cookies. Owned
  groups empty,zeroOOM/pidlimit events. Both namespaces inside disconnected
  LOCAL VM,notexternalphysicalhost. Outer CPU1363.482478user+13.153835system,
  peakRSS2971368KiB,exit0. Native023 SIGKILL cause still unproven.
- README now gives actual frozen024 commands and selected private URLs.
  Full guest reboot persistence passed for this ADAPTED SERVICE ONLY.
  Original-board,uncleanpowerloss,ASIC/mining and full exactmatrix gates open.
- SerialCrc shared bit functions now drive transmit readiness/output. Three
  serial CRC tests pass; exact command byteclocks5,14,23,32,41 test added.
  Old short clock windows failed and retained in clocked-crc-tests001 logs;
  corrected windows include real8shiftclocks/byte without changing expected
  packets. CRC tests002 pass. Shared BM1397 regressions pass in workspace.
- ReceiveFifos now owns source clocked read/CRC/check/write/error states.
  Removed old immediate receive-byte/advance runtime APIs. Moved3 queue
  tests from packet suite into new receive suite; added exact checksum delay,
  clear/IRQ priority and disable-current-output tests. Preserve command-path
  WORK-full gating defect and transmitted-ID sampling at second word.
  Important corrected behavior: disable forces NEXT Idle; current write can
  still happen. FIFOclear discards data but second-write IRQ wins clear.
- Registers.tick now connects actual UART FIFO head/fullness/divisor and both
  core state machines on one clock. tick_core is component FIFO-boundary
  interface. UART errors diagnostic; exact HDL bit widths/timing retained.
  s9io_device tests independently decode TX pins for22literal archived
  packets at divisors0/13 and feed all archived replies as8N1 waveforms at
  divisors0/2/13. Three device tests pass. No ASIC answers generated here.
- Latest workspace166tests in61binaries passed,zero failures/ignored.
  Strict Clippy initially found2unused field accesses in migrated tests;
  fixed and Clippy002 passed. Final scoped tests/fmt pass. Review:
  s9io-device-review-001.json with exact implementation/source/test hashes,
  all25 source members + divider checked. No HDL simulator available and no
  independent-agent review pass. Need docs link check after README changes.
- NEXT: bridge actual firmware to S9IO/ASIC path. QEMU binding absent, AXI
  handshakes and exact legacyHDL/2022bitstream mapping unresolved. Archived
  legacyREADME identifies3S9IO blocks50MHz at43c50000/60000/70000,IRQ
  67/68/84,85/86/87,88/89/90 (GIC absolute numbers,notSPI offsets).
  BM1387 command/work driver in retained source gives1/2/4midstates,63chips,
  114cores,nonce chipbits7:2/corebits30:24, register18ticketfixed32+mask.
  Do not reuse BM1397 opcodes or byte order without exact checks.
- All3 independent agents remain unavailable at unchanged account usage
  limit untilOct25 08:48AM (timezoneunspecified). Do not retry unchanged limit.
  Continue authorized implementation. No successful final response.

BM1387 packet/engine work and storage cleanup:

- Isolated inspector source004 now retains35 exact original members from
  archive dc5f5872...a363. Added source Bitcoin midstate word iterator, work
  producer, core initialization, UIObinding and null-work source. All35hashes
  verified against captured references. No downloaded code executed on host.
- bm1387.rs now decodes exact no-preamble control/work packets and1/2/4
  midstates,retains wire IDs/origin,constructs shared-engine work and encodes
  only recomputed filter-valid responses. Common51bit replyCRC helper reused
  by BM1397. Fixed32bits + exact ticketregister18mask preserved;no override.
- Source byte order established from LE midstate word iterator,rev(),to_be()
  on ARM and FPGA BEbyteoutput. Independent bitcoin_hashes midstate producer
  and full80byte verifier compare2240hashes with RustCrypto compressor.
 20literal controlvectors + single-bit faults,17replyvectors,bitarrayCRC,
  caller-range validation,zero-filter fixed32probability and nonce fields pass.
- Root review caught unsupported inferred byte order for NONZERO nonce origin.
  Fixed:retain opaque4bytes, reject execution with UnsupportedNonceOrigin.
  Known S9IO always emitszero. Engine uses explicit caller RangeInclusive;
  fixed regression searchesgenesisnonce-2..+2 without changing wireorigin or
  filter. Computes5actualhashes andonevalidreply. Notfreshwork/firmware/Core.
- Before reviewfix workspace171tests62binaries passed0failed/ignored. Afterfix
  affected library23 + BM1387integration5 pass,nonefiltered; strictworkspace
  Clippy002passes. bm1387-source-review-001.json pins sources/code/tests.
  Earlierfix-tests001 mistakenly filtered BM1387integration by testname;
  use002which ranall28affectedtests. No independent-agent review pass.
- README gives verifiedBM1387command. Docscheckdue after thisaddition.
  Newmodel is onlycodec/engineconversion. Poweredchain,registers,PLL/open-core,
  physicalnonceownership,UARTresponses,QEMUbinding,freshwork remain required.
  Source prepare_opencore DOESexist atnull_work.rs:146; no missingfunction
  blocker. Noncepartition comments are a firmwareconjecture,notcalibration.
- DuplicateCAScleanup019 removed3505 verified native024duplicates,
  reclaimed1773013380B. Restorejournal
  browser/duplicate-cas-cleanup-019/restore.jsonl. Keepcaptured003/native010CAS,
  native013payload, native024uniquecombinedCAS andbaseinitramfs. Copy byhash
  whenrestoring;nohardlinks. StrictreporterpassedBEFOREcleanup. Originalarchive
  untouched;compactnative024evidence remains complete36MiB.
- BM1397 originalprobe033stillstops on write20=1. Communityprovider reference
  defines register20reset0000FFFF but gives no bitfields/electricalsemantics.
  Do not invent a clock-lock success from registerstorage. Next fullASIC
  work must connect real state/timing and keep unresolved physicalevidence.

S9IO AXI wrapper review checkpoint:

- Added clocked AXI handshakes, simultaneous CPU/core FIFO strobes and
  synchronous FIFO output mux. Preserve archived pending BVALID/RVALID
  defects, pre-clock FULL decisions, clear priority and unknown first read.
- Six AXI tests and one extra receive arbitration test pass. Full workspace
  151 tests in 36 test/doc binaries pass; zero failed/ignored.
  Strict workspace all-target Clippy passed. Review: s9io-axi-review-001.json.
- No independent review available. No HDL simulation or guest transactions
  through S9IO yet. Exact legacy/2022 bitstream mapping remains unresolved.
- NEXT: bounded idle-clock advancement with equivalence checks, then QEMU
  binding. Keep ordinary controller clocks separate from hash-rate caps.

Idle clock and file-lock fix checkpoint:

- S9IO idle_limit/skip_idle preserve 50MHz clock phase without per-clock
  evaluation in stable states. Four equivalence tests pass, including baud
  changes, timer wrap, queued commands, live RX/IRQ and u64::MAX spans.
  Review: s9io-idle-review-001.json. QEMU binding still next.
- Full --all-targets run exposed queue reopen failure at queue_behavior:276.
  Failure retained in workspace-all-targets-after-s9io-axi.log. Plain File
  close can leave flock held by inherited pre-exec child descriptor. Actual
  failing child identity unknown; deterministic real-fork test reproduces
  the mechanism. New ExclusiveFileLock unlocks in owner PID only. Applied
  to queue, store, worker root and endpoint; same validation/exclusion.
- file-lock-fix-001 has25 passing scoped tests. Strict Clippy and current
  workspace --all-targets pass183tests/64binaries,zero failed/ignored.
  Review: file-lock-review-001.json. Native024 remains historical frozen
  execution, not evidence for changed runtime binaries.
- Earlier workspace-after-s9io-axi was default targets:151tests/36binaries.
  Do not confuse that count with the all-target check above.
- No independent review and no new exact-target firmware mining pass.

QEMU legacy S9IO integration checkpoint (active work):

- New md-sim-qemu-core static library reuses firmware S9IO state and existing
  AXI-IIC/PL353 Rust code. Standalone backend host-std feature omits its
  no_std panic/personality shim only when linked with standard Rust runtime.
  Original no_std build and fatal-unwind C test remain tested separately.
- C QEMU interface uses50MHz virtual core clock, bit RXD/TXD,9IRQ routes,
  atomic MMIO. CPU AXI channel stalls are explicitly not modeled. No clock
  advances from a poll. Unsupported controller state halts the VM; unknown
  first FIFO read returns memory transaction error. No ASIC reply invented.
- Added explicit md-sim-s9-legacy-io-development machine. Inherits existing
  unresolved M69A development storage fixture. Three blocks at43c50000,
  60000,70000; fixture build ID74657374, source version00900200. This is NOT
  compatibility evidence for selected2022 bitstream. ASIC chain absent.
- New isolated source005 retains36members, including system.tcl SHA
  650d3c550e8240218493b85c848182406347249661e785c87ea58cff2695d44d.
  It resolves README genericbase+2 inconsistency: IRQ_F2P6,7,8 maps67,68,84;
  nextblocks85..87/88..90. Original archive remains unchanged.
- Isolated QEMU033 build passed.034 adds stable/machine/s9io5..7 object paths.
  Binaries033=31a674389f79831eb713598835760728eaea803c636f8f33451187e931907138;
  034=ce8b234b05f15238a415902da51a520f4a808dd5d4b20ed4e4ab5701a290f289;both9387928B.
  Compact build reports retained in qemu-s9/backend-build-033 and034.
- qtest001/002 failed GIC pending reads; retained. Exact QEMU qtest uses
  nonsecure attributes, so Group0 pending reads are masked. clock_step0 is
  also explicitly rejected by QEMU. Corrected harness uses unique GIC input
  observer and clock_set same timestamp; security model unchanged.
- legacy-s9io-qtest-003 PASSES1363 exact command responses plus18 exactIRQ
  raise/lower events, all9routes. All3 actualTXpackets match archived
  [54,05,00,00,19] hex and exact161*(divisor+1)*20ns start intervals at
  divisors0,1,13. Literal response input, FIFO/readback/error clear pass.
  This is component testing, not firmware execution/mining or CPU exception
  delivery. Core-combined-iic-qtest-001 also passes12responses on033.
- S9IO C ABI ASan/UBSan passes including actual computedCRC and serial input.
  LeakSanitizer unavailable under host ptrace; failure retained001, tests
  with detect_leaks=0 pass003. Old IIC personality test cannot call std
  personality with invalidNULLcontext; retained failure and explicit build
  feature split. Both standalone fatalshim and combined ABI tests pass.
- Full workspace before last qtest harness edits:183tests/65binaries passed;
  Clippy passed. Current qtest real003 passes. Standalone and host-std unit
  suites pass. Need final current checks/source review/docs after last fix.
- Verified duplicate033/034 CAS input cleanup journal:
  qemu-s9/duplicate-input-cleanup-001/restore.jsonl. KEEP031CAS. Restore by
  copying, never hardlinking, before historical full-input reaudits.
- ACTIVE: UBSan QEMU build035 at/tmp/md-sim-qemu-backend-035-ubsan, tool
  session8855. Next run same legacy qtest against035, review/fix, then move
  into guest-driven controller and ASIC chain. No exact-target gate pass.

QEMU sanitizer and next-path checkpoint:

- Legacy qtest003 passed on034 with exact IRQ input observation. Default
  nonsecure qtest reads cannot inspect Group0 GIC pending state. CPU exception
  delivery remains unverified. Source qemu-gic-source001 retains exact GIC
  and SLCR source. New qemu-clock-source001/kernel-clock-source001 inspection
  captures clock/reset dependencies; inspect their result before use.
- 035UBSan compiled all1396objects but linker hit64MiB file limit before
  strip.036raised128MiB but isolation guard correctly rejected it before
  acquiring/executing tools. Both failures retained, not passes.
- Added require_emulator_build profile with128MiB max output; all original
  firmware-run caps unchanged. Builder and guard shareconstant. Removed
  unused standaloneIIC archive input from combined build. Exported executable
  limit remains28MiB.037UBSan running tool session54267 under/tmp/md-sim-qemu-
  backend-037-ubsan. Run legacy qtest plus prior IIC/NAND regressions next.
- Standard cargo build changes staticlib identity relative to cargo rustc
  --print native-static-libs invocation.034tested be454e99...49681;standard
  rebuild was13e070...b6f5e9e before latestconfinementchange.037 captures its
  own new exactlibrary. Do not reuse034test as evidence for a changed build.
- Added s9_backend_boot legacy-io diagnostic option with explicit unresolved
  legacy/2022 mapping and noASICchain. It savesactualguestS9IOtransaction
  counts/first32lines. NOTYETRUN. After037passesqtest, run archived kernel/
  ramdisk unchanged with this declareddevelopmentprofile; nooriginalgate.
- New S9IO.md documentsverified034componentcommands and remainingclock/reset
  coupling. READMElinks it; guide linkspass341. CABI READMEcommands tested.
- Rootworkspace183tests/65binaries passed before latestconfinement/boothelper
  changes. Re-run affected/fullchecks after latestfix. Independentagents still
  unavailable. No exactphysicaltarget mining/Coregatepass.
- Duplicateinputcleanup002 removes onlyverified035/036CASduplicates;KEEP031
  CAS andrestorejournals001/002. Originalarchive unchanged.

S9IO sanitizer and original SD boot follow-up:

- Backend037 UBSan passes. Binary e24da8d8fa89966f88d91e6206b886c1e7fec161e473aab691c0cadc424cdd79,
  19492960 bytes. Combined library206f1e19ee91714145d5547f864827b0c89265579da2ba35315bb5cb39a354df,
  31091312 bytes. Compact build report retained under backend-build-037-ubsan.
- legacy-s9io-qtest-004-ubsan passes1363 responses and18 IRQ transitions.
  combined-iic-qtest-002-ubsan and combined-onfi-qtest-001-ubsan pass.
  Current C ABI ASan/UBSan checks pass with leak detection disabled for the
  previously recorded tracing limitation. Source review s9io-qemu-review-001.
- Direct-kernel legacy-s9io-guest-001 hit trace quota; retained failure.
  guest-002 focused S9IO trace avoids quota but exits137 at52301ms. Guest
  kernel/procd/network run; no S9IO transaction. No CPU-limit cause is proved.
  No SD device in that direct-kernel diagnostic; it logs missing baseline SD.
- Full original SD/SPL probe legacy-s9io-spl-001 ran120051ms then timed out,
  no trace truncation, no S9IO transaction. Original SPL, U-Boot, FIT checks,
  kernel, extroot on mmcblk0p2, procd/ubus and network execute. Kernel reports
  GPIO bases897(input) and884(output), but these do not prove device behavior.
  UBIFS rootfs missing on blank development NAND remains visible. No mining.
- New optional console=SCRIPT on s9_boot_bundle records a bounded immutable
  script, activates the actual normal firmware console and waits for a guest
  completion line. It does not replace init or change original image bytes.
  Shared SerialGuest moved from s9_native_service into serial_console module;
  retained reset/completion tests pass. First build exposed a missing Read
  import after extraction; fixed, second build is in progress. Need real
  console run, native shared-driver regression, Clippy/tests and source review.
- Agents remain unavailable due account usage limit until Oct25 08:48AM.
  No independent review pass. No exact physical miner passes required gates.

Serial console and GPIO implementation checkpoint:

- Shared serial_console Rust module now serves s9_native_service and the
  optional s9_boot_bundle console=SCRIPT path. Original normal init is retained
  for the latter. Console activation first requires an actual executed command
  sentinel, then the immutable selected script. Cleanup kills/reaps ownedQEMU.
- legacy-s9io-console-001 exited0 after110819ms, no truncation. Actual archived
  BOSminer0.9.0-5eb2c5fe and BOSer port80 service start. BOSminer reports
  enabled_chains=[] and fan autoconfig[0,0,0,0]. No S9IO transactions. No mining.
  Guest registers current2022 chain6..8 UIO blocks at43c00000/10000/20000;
  IRQtx/rx/cmd62/63/64,65/66/67,68/84/85. This differs from legacy system.tcl.
- Original normal SD execution still uses declared SPL entry/secondary CPU
  adaptations, blank M69A NAND development fixture, fixed fabric clock and no
  ASIC chain. It is NOT an original-board or per-target pass.
- storage-warm-shared-001 passed14462ms, no truncation. SameQEMUPID4, actual
  kernel restart line, fresh UCI value114 persisted then restored100. This is
  the existing adapted source-service storage fixture, not physical boot media.
- serial-failure-shared-001 deliberately runs exit23; guest sentinel reports23,
  isolated driver exits1 after5547ms. Failure is not relabelled as success.
- console-002 used absent od and lacked set-e. Despite final shell status0,
  its property extraction FAILED. console-003 uses hexdump and a fail-fast
  subshell; exited0 after44369ms. Guest DT declaresGPIO input9/output13,
  one channel, noIRQ, DATAdefaults0, TRIdefaultsffffffff. Raw output interleaves
  echoed input/kernel text; preserve it, do not silently normalize evidence.
- serial-console-review-001.json records exact source and run hashes, failures
  and limits. Workspace after shared-driver extraction passed183tests/65binaries;
  Clippy passed. Later GPIO/newlayout edits have their own checks in progress.
- Added Rust axi_gpio register model and QEMU C ABI/binding. Fixed input/output
  wiring, actual DATA/TRI state, masks, reset, external pin levels, absent
  channel/IRQ behavior and whole-word writes. PG144 says WSTRB is ignored.
  3Rust behavioral tests and C ABI ASan/UBSan pass. NoLeakSanitizerclaim.
  GPIO.md lists source references and explicit settled-input/atomic-MMIO scope.
  Synchronizer, AXI timing and fabricclock/reset coupling remain work.
- Exact archived gpio-xilinx.c member d50659f2f29149d0b7ddc6fab303d61c3c1dfd38ba6ec51a572e87ce5f0cb867,
  16760B, retained kernel-gpio-source-001. Sameinspection capturesclk-divider.c
  69cc450333b0189ff86f1757c5fed0f277b95042ae6f1d5552c974b7c7911c3f and
  xilinx_devcfg.c60b83d922a5ed031cd6fec6e60506d91a0314493b56417b620167c37fab3708f.
- Public AMD PG144pages and vendor revCtrl AXI GPIO HDL were read through web.
  Hostcurl docs.amd.com failsDNS, retaineddownloadlog001. Webreferences are NOT
  pinned archived byte inputs; exactsourcecapture/bitstream correspondence open.
- Backend038UBSan PASSED190988ms. Binarybab45352bc5c118833f57b23a9718692b512b1b2b60244039873248358ad720c,
  19506720B. Library67b3e88f1ad7d15c6ec9de13c40f622d1ce49e06aabb469df4f3433b98b91437,
  31191854B. New md-sim-s9-gpio-development fixture inherits legacyIO andNAND;
  defaultinputpins all low, noASICchain. Compactreports backend-build-038-ubsan.
- gpio-qtest-001-ubsan passes25responses+6actualoutputpin transitions.
  legacy-s9io-qtest-005-ubsan passes1363responses+18IRQs; combined-iic003 and
  combined-onfi002 also pass on038. No firmwarerun-through-GPIO claim yet.
- InitialGPIOClippy rejected unclear test-expression precedence. Parentheses
  fixed. Latest lint/tests run sessions42576/6968; fmtcheckpassed.
- Cleanup journals duplicate-input-cleanup003(037CAS) and004(038CAS) each
  reclaim30verifiedduplicates571697557B. KEEP031CAS, originalarchive, journals
  and all unique objects. Restore bycopy, neverhardlink, forfullhistoricalaudit.
- ACTIVE: Backend039UBSan build session34169 under/tmp/md-sim-qemu-backend-039-ubsan.
  New sd2022-io patch maps reused legacycore to the actual2022 DTaddress/IRQ
  layout. Explicit development hypothesis, not compatibility proof. Newmachine
  md-sim-s9-sd2022-io-development. qtestmode sd2022-s9io reuses sameprotocol
  regression with newaddresses/IRQs/linknumbers6..8. Boothelper sd2022-io and
  optional presence-pin-fixture=HEX record externalpin teststimuli; noASICchain.
  Next: inspect039report, run allaffectedqtests, then actualarchivedSD console
  probe with explicit dangling-board pinfixture to observe real initialization
  and next device failure. No fake power-good, ASIC reply or nonce permitted.
- No exactphysicaltarget mining/Coregate passes. Independentagents unavailable
  due accountlimit. Do not send a successful completion response.

Default-enabled S9IO scheduling investigation:

- Backend039UBSan passed190678ms, exit0, no truncation. Binary
  f75b1778379fc480645518584744951d588b3c037ae0ec73340c3f773e98592a,
  19501824B. Five qtests passed: sd2022-s9io001 and legacy006 each1363
  responses+18IRQs, GPIO00225responses+6pin transitions, IIC004 andONFI003.
  Workspace GPIO/layout checks passed186tests/66binaries, Clippy andfmt.
- sd2022-presence-console-001 timed out120043ms, no truncation. Actual
  originalguest made29GPIO accesses and its first4S9IO MMIOtransactions:
  chain6 CONTROL read0/write0/read0/write8. Only consoleactivation sentinel0;
  presence script never completed. No boot/mining pass. Exact explicit
  external input pinfixture=e0, noASICchain, inheritedlegacycore hypothesis.
- Reproduced defaultenabled WORK_TIME=1 idle_limit=0 in a new regression.
  s9io-empty-regression-before-001.log retains the failing assertion.
  Empty Wait/Check states generated a50MHz QEMU callback stream.
- Implemented analytical empty-queue advancement. It preserves the exact
  24bit timer, ACK priority, Wait/Check phase, UART phase and register/pin state.
  Optimization is forbidden for pending reset bits, active serial paths,
  partial work/command queues, pending AXI handshakes or busy checksums.
  Disabled timer and ordinary nonempty paths retain the prior implementation.
- 8 idle tests now pass in s9io-empty-tests-003.log. They compare every
  following edge and observable effect with ordinary clock-by-clock execution,
  include810timer/divisor/phase combinations and fullu64 spans, and preserve
  literal protocol/waveform tests. One test initially assumed a WORK_TIME
  write resets the live timer; it does not. Corrected that test setup with
  an explicit IPreset; retained failing002log. Formatting fix applied.
- Added the defaultenabled one-second virtual advance to both QEMU layout
  tests and theC ABI regression. Fullworkspace/lint/releasebuild running
  session31633. Next stage library, Csanitizers, isolated040UBSanbuild,
  affectedqtests, then repeatoriginalpresenceprobe onnewbackend.
- Agents remain unavailable due previously observed account limit. No
  independentreviewpass. All exactphysicaltarget completion gates remain open.

Empty-loop fix and console transport follow-up:

- Backend040UBSan passes191372ms. Binaryb0c481958498055aeeda5d298e796329c1d2c7204d72d50d52f872c42c427d9b,
  19506560B; librarydb848dcfb73c469e1e0295003e8c581a13835a7561d433e86bcdb09e8fe703f6,
  31190146B. Five affectedqtests pass, including new1second defaultenabled
  span on eachS9IO. Retained old039 test sd2022-s9io-empty-before-001 fails
  after5responses with exit137 at5036ms under5secondCPUbudget.
- New qtests: sd2022-s9io002,legacy007,GPIO003,IIC005,ONFI004 allUBSan.
  CABI ASan/UBSan pass. 9idle equivalence tests include long24bit timer wrap
  and fullu64 spans. Workspace190/66 passed before the added long-period
  and console tests. Allchanges now undergoing finalworkspace checks.
- Self-review s9io-empty-loop-review-001.json records exactsource/input hashes.
  Not an independentreviewpass. No calibration or physicaltargetclaim.
- OriginalSD rerun sd2022-presence-console-002 still timed out120031ms,
  no truncation. Only4GPIOinitMMIOs, zeroS9IOtransactions, kernel log reaches
  ~15.8seconds. Completion suffix visibly loses its beginning after the
  script'sclosingparenthesis. This rerun does NOT establish firmware progress.
- Serialdriver now wraps script and completion command in onePOSIXcompound
  group. Shell must parse suffix before it starts any sleep/busycommand.
  New /bin/sh oracle checks partialgroup produces no script output, complete
  group preserves success and exit23. No guest shell syntax modification.
- ACTIVE: originalSD samepresencefixture rerun003 session13428 with newdriver;
  originalbackend040 unchanged. Fullworkspace/lint/fmt session65284.
  Next inspect003, then run shared-driver guestnegative andwarm-reboot checks.
  If003still fails, use per-second guestprogress script to isolate startup.
- Cleanup005 reclaims30 identical039CASobjects571697557B after hashchecking
  both copies and fsyncingrestorejournal. KEEP031CAS asrestore source.

Serial completion, exact FPGA inputs, and next preparation work:

- Grouped console003 completed but exposed an activation brace error. The
  driver now sends the activation newline first and waits for the real shell
  prompt. Run004 completed106827ms without syntax errors. Run005, using
  bundle006, also completed. Both retain the actual BOSminer panic: required
  S9IO1.0.2, implemented legacy0.2.0. These are diagnostic passes with failed
  miners, never exact-target passes.
- Serial negative guest test serial-failure-framed-001 returns exit1 after
  actual script status23. storage-warm-framed-001 passes the same-process
  reboot/persistence checks. Four serial unit tests pass, including a real
  POSIX shell incomplete-group oracle. See serial-console-review-002.json.
- Sources braiins-io-source-002, braiins-io-binary-source-001,
  braiins-docs-source-001 and dcent-s9io-source-001 are hash-checked isolated
  extractions. s9io-102-evidence-001.json records conflicts and remaining work.
  The public zynq-io source targets G9/G19. Its archived2019S9 binary differs
  from the selected2022 image. DCENT_OS documents an S9-specific1.0.2 contract
  and200MHz serial clock; correspondence to selected FPGA logic is unproved.
- Rust bitstream container decoding now enforces bounded gzip expansion,
  CRC, one member, exact Xilinx tagged header and payload size. Three tests
  pass; strict firmware Clippy and formatting pass. It does not interpret
  configuration commands or establish model compatibility.
- Bundle006 preparation passed513ms. All original image, partition, stage
  and boot-file identities match005. Derived FPGA980532de...5a4421 and
  configuration49d0faa4...bea78 match an independently isolated gzip run.
  Full identities: original-zynq/sd-bundle-006/fpga-assessment.json.
- Next: extend exact locked SD preparation to other archivedS9 releases,
  execute their real boot paths, and resolve controller/peripheral gaps.
  The2022 target remains required. No version override or fake device reply
  was added. All exact mining and full-matrix gates remain incomplete.
- Backend040CAS duplicate cleanup006 retains restore maps to protected031CAS.
  Do not delete031CAS. Agent account limit remains unchanged; independent
  review is unavailable, not passed.

Two exact SD inputs and a second original-firmware failure:

- s9_boot_bundle now selects pinned sd2022 or sd2020 archive profiles.
  Inspection runs first in isolation; boot member hashes are then pinned.
  Original2022 assertions remain. Boot also rejects mixed image IDs/sizes,
  false load/entry/source offsets, and conflicting boot-member metadata.
- sd2020-inspection-001 passed410ms; sd2020-bundle-001 passed420ms.
  Whole imagea195af10...9a574, SPLbd4a2992...90c18f,
  U-Boote50ca9b2...876e72. Full identities are in their retained manifests.
  FPGA569597aa...564713 dates2019/12/02; it differs from both archived2019
  zynq-io-bin S9 and selected2022 FPGA. No version compatibility inferred.
- sd2020-console-001 executed actualSPL2016.03-gecf87166e1, U-Boot and
  Linux4.4-g4e459eb5fbb6. Bootloader CRC32/SHA1 verification passes all FIT
  components. Kernel panics atPC0, LRnand_write_subpage_hwecc+0x94 while
  ubiattach creates metadata on the empty M69A development fixture.
  Exit1 after111821ms, no truncation. This is failed boot, not acceptance.
  Archived reference pl353_nand.c sets hwctl=NULL for non-on-die ECC; exact
  source build correspondence is not established. Do not fake ECC enable.
- Workspace checks pass197tests/66binaries, fullClippy/fmt; guide links pass.
  s9-multi-image-review-001.json is a root review, not independent approval.
  README has verified inspection/preparation commands. No exact target passes.
- ACTIVE: source inspector16025 extracts DCENT BM1387 driver and lists ASIC
  source members for the1.0.2 packet contract. No1.0.2 version override was
  implemented. Next implement only evidenced interface differences and
  retain unvalidated timing/model identity explicitly.

S9IO reference profile and backend042:

- DCENT source003 retains exact BM1387 driver, FIFO/protocol builders and
  PIC1704 protocol. Member hashes are in references.json and S9IO-V1.md.
  Its top-level BM1387 catalogue has conflicting chip/work-ID statements;
  driver code is used as the reference. Acquired instructions are not active.
- Added separate Reference1_0_2 BM1387 profile: version00901002,200MHz
  serializer, ENABLE-low physicalTXD BREAK, stored BM139X control bit with
  explicit unsupported-mode failure, and rejection of destructive midstate
  clear/disable after actual UART traffic. Legacy behavior remains separate.
  Common FIFO internals/reset defaults are inherited inferences, not selected
  FPGA equivalence or calibration. This profile cannot pass an exact target.
- Three new Rust tests pass. First command test incorrectly wrote into the
  pending enable/reset queue clear; corrected setup waits for that real clear.
  Failedlog s9io-reference-tests-001 retained. Review also corrected returned
  TXD/started events to represent the gated physical output.
- Workspace200tests/67binaries, strictClippy/fmt and CABI ASan/UBSan pass.
  NoLeakSanitizerclaim. Model staticlibrary6b695a9865a812e8f764f0453fa31d6466ade390a33777086efed7fdf23dc907,
  31254452B. core-sources-041 captures154 source/build files and provenance.
  Prior040library remains in its unique CAS objectdb848...703f6.
- Backend041 failed on storage quota before any build report/result. Failure
  and cleanup pointers are retained under/tmp/md-sim-qemu-backend-041-ubsan.
  After verified cleanup,042UBSan passed193460ms with all startup checks.
  Binary91134a6b8150d25e8a96bcbf572512afb250a7e98247507ae065358fb246b8de,
  19502080B. Compact reports areqemu-s9/backend-build-042-ubsan.
- Six compiledqtests pass: reference-v1-001, legacy008, sd2022-legacy003,
  GPIO004, ONFI005 and IIC006 (allUBSan). EachS9IO checks1381protocol replies
  plus18IRQ lines. V1 uses reference divisors108/7/3 and independent8N1 input.
  None executes guest firmware or proves mining. Exactassessments retained.
- CLIreview found boot console failures returned command0 after saving a
  result with exit1. Fixed to require exited/status0/untruncated for requested
  console diagnostics. Actual sd2022-script-failure-001 observes gueststatus23,
  returns driver1,42278ms, no truncation. Original kernel/services executed.
- ACTIVE:83987 runs original2022 bundle006 on042 with sd2022-v1,
  explicitpresencefixture=e0, same presence script and added AXIIIC tracing.
  Outputqemu-s9/sd2022-reference-v1-console-001. Inspect next actual failure;
  do not infer miner success from diagnostic script status.
- Quota cleanup: removed only stale AXIIIC incremental cache and verified
  duplicates. duplicate-run-input-cleanup-001 removes102objects469487019B;
  KEEPsd2022-presence-console-004 input-store as restore source.
  duplicate-input-cleanup-007 removes30 failed041 duplicates571697557B;
  KEEP031CAS. duplicate-sd-input-cleanup-001 removes34 repeatedSDobjects
  3760193536B; KEEP004 andsd2020-console-001 image objects. Journals are fsynced
  before removal; restore bycopy, neverhardlink. Originalarchive untouched.
- Emergency idle bash70212 remains open. Use it if quota prevents newexec.
  No exact physical target or fresh-mining/Core gate passes. Independent
  agents remain unavailable under the unchanged account usage limit.

### Reference profile reaches power and cooling interfaces

- Reference probe001 timed out at120039ms before any S9IO access. It reached
  the original shell and network setup. This is a failed diagnostic.
- Added guest uptime/process snapshots to console-presence-progress-001.sh.
  Added serial child CPU and elapsed-time records. Values come from
  getrusage and a monotonic clock; RSS is explicitly a cumulative maximum.
  Four serial tests, affected Clippy and release builds pass.
- Probe002 uses the same originalSD2022 bundle006 and042 backend, reference
  profile and explicit e0 presence stimulus. It completes its diagnostic at
  119932ms with exit0 and no truncation. Whole command125.128s; emulator
  95.777484s userCPU,1.502136s systemCPU,119.919s wall. Multiple Dropbear RSA
  generators appear in the guest process snapshots. No timing bypass added.
- Actual firmware reads version00901002 on chains6/7/8, then initializes the
  controller interfaces. 57S9IO MMIO transactions and61IIC address NACKs are
  retained. BOSminer shuts down: zero fans meet the minimum of one; board
  power controller addresses55/56/57 have no connected slave.
  Its printed controller build date decodes the explicit74657374 fixture ID;
  it is not an actual build date or evidence of FPGA equivalence.
- Probe002 assessment.json hashes its inputs, logs and result. Diagnostic
  exit0 does not pass miner acceptance. Mining and the original-board gate
  remain incomplete.
- Boot diagnostics now use the existing bounded live trace collector.
  Host-owned log descriptors are not visible to the guest. Affected Clippy
  and release build pass; next actual boot must check the retained live logs.
- Next: implement fan and power interfaces from pinned evidence. Public
  DCENT fan claims and legacy SVD differ; preserve that distinction.
  Isolated extraction dcent-fan-source-001 is active in session43648.

### Fan interface and exact member extraction

- Added Rust fan registers and motor motion, C ABI and QEMU binding.
  Native source PWM scale is0..100. The archived DCENT fan.rs hash is
  79c8819e086c18b92807a7ba8cf53c0d8bea05a1e2e41f68b2da82b7aaf474e7.
  It contradicts that project's older0..127 architecture text.
  The component contract is simulator/backends/qemu/FAN.md. It states every
  assumption: uncalibrated900..6000RPM motors,12000RPM/s slew, two pulses per
  revolution, one-second sample windows, two PWM channels, only tach1 wired.
  Default motors are absent. No successful fixed tach value is injected.
- Six Rust tests cover analytic pulse counts, irregular scheduling,
  independent PWM, power loss, stall, reset and invalid requests.
  Exact integer integration handles fractional ramp completion without
  schedule-dependent rounding. Workspace206tests/68binaries and strictClippy
  pass. C ABI ASan+UBSan pass with LeakSanitizer disabled due environment.
- Source snapshot core-sources-043 has183 files. Staged model library SHA-256
  c2d986c9eacdcf6d2039e668d7748326ee3a6d95ea0dc64dae18da647da000b3,
  31276358bytes. Prior042 library remains in042CAS as referenced by snapshot.
- Backend043 failed quota during input preparation. infrastructure-error.json
  retains the failure. After cleanup,044UBSan builds and passes all startup
  checks in185870ms, exit0, untruncated. Binary54781c7e099821190927ba8be21052de4738a8fb604edd9718326e51ecbc0ed5,
  19520320bytes. Compact reports areqemu-s9/backend-build-044-ubsan.
- Seven compiledqtests pass: fan001(25responses), fan-empty001(25),
  reference-v1-002(1399includingIRQs),GPIO005(31),ONFI006(237),IIC007(12),
  legacy-S9IO009(1399). No firmware claims follow from these component tests.
- FullSD firmware diagnostic sd2022-fan-console-001 passes its script at
  127202ms,exit0,untruncated. QEMU CPU95.969769s user+1.040025s system,
  wall127.186s. It uses explicit presencee0 andfan-fixture parameters.
  Native BOSminer logs FANautoconfig[0,900,0,0], writes100 then0 to both
  PWM channels, and remains running at the service snapshot.
  All three hashchains still fail at absent PIC addresses55/56/57.
  84actual fanMMIO transactions are retained. Assessment hashes inputs/logs
  and distinguishes marked devmem diagnostics from native driver access.
  Live trace files exactly match decoded result streams.
- Based on measured wall/CPU use, boot wall budget now permits30..240s,
  with unchanged110s CPU ceiling. Default30s remains. Guest clocks,
  ordinary timing and firmware bytes are unchanged.
- Storage recovery: own Cargo debug examples and incremental caches removed,
  with inventory in cargo-cache-cleanup-001.json. This did not by itself
  remove the /tmp quota failure. duplicate-input-cleanup-008.json removes
  55verified objects721017909B from042/failed043, restored from031.
  duplicate-input-cleanup-009.json removes148verified objects2897031557B
  from020/022/024/026/032. It protects all absolute paths in earlier cleanup
  journals. Unique models, binaries, originals and unrelated /tmp projects
  remain. KEEP031canonicalCAS and042unique prior model library.
- PIC input research: braiins-pic-source-001 anddcent-pic-source-001 finish.
  Braiins power/firmware.rs hash54145acc99d975db9b1a4652462e59fb474ac7008f13a91b090d70a611933e3c.
  App load range0300..0f7f inclusive,3200words/6400bytes, big-endian words.
  Source app text hash ec84da07ef98665c6258cc531dc2d74aa8be7018925c041325ca233aec52b46a,
  19200bytes, identical in archived bosminer andbmminer packages.
  Legacy power.rs explicitly states watchdog about10s, heartbeat1s, reset
  500msNACK and100ms flash/voltage delay. These are reference timings, not
  calibration. DCENTpic1704 protocol is a different interface; do not reuse
  it just because its filename names a PIC.
- Found and fixed actual preparation bug: native-service extract accepted
  7zip exit0 with zero bytes for nonexistent/etc/bosminer/hash_s8_app.txt.
  Historical pic-extract-001 is failed evidence despite its old CLI0.
  New isolated helper checks one exact regular member, disables wildcard
  matching, rejects missing/duplicate fields and verifies extracted size.
  Bounded reads and no guest filesystem export are preserved.
  One parser regression, affectedClippy and release build pass.
  Real missing-member002 returns driver1 and exact-member-not-found.
  Real pic-extract-002 findslib/antminer/hash_s8_app.txt with the same
  ec84...52b46a hash in pinned2022 ramdisk2bd49c70...47154b3.
  member.json pins parent, output and result. No PIC execution claim.
- Next: implement the evidenced PIC loader/runtime path with flash,
  parser state, real timings, power and watchdog state. Preserve missing
  baseline/calibration/bootloader evidence as gaps. Reuse a suitable existing
  PIC emulator if instruction execution is needed; gpsim/gpdasm are not
  installed. Do not invent a CPU emulator or report expected version3 as
  measured execution. Exact target, mining/Core, and independent-review
  completion gates remain open.

### PIC component started; QEMU wiring is next

- Added crates/md-sim-firmware/src/pic16.rs and six passing component tests.
  Tests use the exact application under simulator/firmware/pic16/ with a
  provenance manifest. The copied19200-byte input is unchanged; originals
  remain archived. Independent bitcoin_hashes SHA256 confirms both raw text
  ec84...52b46a and6400 decoded big-endian bytes
  d138aefed416e18a7c5f2a374ef99148c745f7d576b3821e615225fffad1d35b.
- Tests cover real prefix grammar across STOP, the documented broken55,55
  rematch and zero flush, full3200-word erase/write/readback, interrupted
  pending writes, persisted committed cells, invalid addresses and time
  overflow, voltage state and heartbeat expiry. Logs pic16-tests-002 and
  pic16-clippy-002 pass. Earlier five-test log001 is retained.
- Root review fixed flush handling: a transfer buffer can contain arbitrary
  bytes during recovery, so14-bit validation occurs at programming time.
  It also made delayed operations atomic on deadline overflow, disallowed
  voltage enable before a known voltage command, and allowed explicit
  external reset after unknown autonomous watchdog recovery.
- The model explicitly selects a protocol contract for the known app hash.
  It does NOT execute PIC instructions. Unknown/damaged app hashes fail.
  Erased loader/calibration storage is an explicit development baseline.
  Watchdog expiry disables power and reports unspecified recovery; later
  heartbeats cannot turn that into success. Unsupported commands fail.
  Read bytes after the first in a transaction are marked undefined; the
  recorded driver discards them. Timing uses uncalibrated driver waits.
  Full limitations are simulator/backends/qemu/PIC16.md.
- QEMU binding for this component has NOT yet been implemented. Next add
  small C ABI and I2CSlave binding with explicit app input. Existing
  source-inspection-008/text/include/hw/i2c/i2c.h has the pinned QEMU API.
  The parent machine creates AXIIIC withsysbus_create_simple in
  s9-axi-iic.patch; capture its returned device, get child bus"i2c", and
  attach separate PIC instances at55/56/57 only on a new development machine.
  Keep previous profiles unchanged. Add real compiled bus tests, then run
  the archivedSD2022 path. Allow enough bounded trace bytes for original
  driver's many single-byte I2C reads; do not discard required transactions.
- Backend044 and its staged c2d986...000b3 library predate PIC code. Do not
  claim PIC support for044. Preserve that library from044CAS before staging
  a later model build. Exact-target mining and all completion gates stay open.

### PIC binding in progress

- Added owned Rust C ABI and QEMU I2CSlave binding. The new development
  machine attaches separate instances at55/56/57. Application text is
  explicit and bounded; decoded words must match the pinned image.
  Busy and empty replies NACK. Unsupported states stop the VM. A virtual
  timer commits pending operations and enforces the watchdog without bus
  polling. The power-enable output is exposed but has no ASIC connection.
- Added explicit `pic-contract` boot option, input hash checks, adaptation
  metadata and32MiB bounded trace allocation. Existing profiles remain.
  Added compiled-bus qtest transactions with literal app word3183, distinct
  controller pointers, runtime version/voltage readback, busy NACK, and
  observed power-enable output transitions. These tests are not firmware
  acceptance. Their execution is pending backend045.
- pic16-ffi-001 and002 pass strict GCC warnings and ASan/UBSan.
  LeakSanitizer is disabled due to the established ptrace limitation.
  They check exact app identity, read latch, transaction boundaries, reset
  busy interval, voltage, watchdog cutoff and error status.
- pic16-workspace-clippy-001 passes. pic16-workspace-tests-001 passes
  186tests across43 result groups; this command omitted example tests.
  Do not confuse it with the prior206-test all-targets run. An all-targets
  check is still due after the current integration fixes.
- Source snapshot core-sources-045 retains195files and exact model library
  25a994b807580bffb43cf60dd543496fb00af4c0b895d3b92d47b155fe490451,
  31352682bytes. Prior c2d986...000b3 is verified and preserved in044CAS.
  The qtest output-pin assertion was added after this snapshot; the probe
  records both its main source and helper hashes, plus its actual binary.
- Backend045UBSan isolated build is running. Command:
  target/release/examples/build_qemu_backend /home/user/src/md-sim
  work/simulator/qemu-s9/samurai-build-003
  /tmp/md-sim-qemu-backend-045-ubsan
  /tmp/md-sim-qemu-backend-031 ubsan.
  Driver log is checks-2026-10-20-002/pic16-qemu-build-045-driver.log.
  Next run actual compiled PIC qtest, fix failures, then run SD2022bundle006
  withsd2022-v1,fan-fixture,presence-pin-fixture=e0,pic-contract,budget240
  andconsole-pic-check-001.sh. No new firmware acceptance claim yet.

- Backend045 built successfully in181231ms. Binary:
  515747b664b888f666896fe35fb594a2e52f69e888918d4e44f82ce4bb2710c2,
  19566472bytes. All patches and six startup checks pass underUBSan.
  Compact reports are backend-build-045-ubsan. Fan, empty-IIC and S9IOv1
  compiled regression tests pass against045.
- pic16-qtest-001-ubsan FAILED and remains retained. Two causes:
  the test omitted RX_FIFO_PIRQ15 and read before sufficient data arrived;
  AXI IIC charged a whole byte interval for its internal receive-count word.
  The first caused clock stretching and empty FIFO reads. Later commands
  were interrupted by test resets; enabling an unset voltage correctly
  stopped the PIC model with code7. No firmware was involved.
- Fixed the test's receive watermark. Fixed the Rust IIC engine so the
  receive-count descriptor and first data transfer use one step. The
  count is not a wire byte, per AMD PG0902016 TX_FIFO description:
  https://docs.amd.com/api/khub/documents/qDTQFhZJdUT~2BwH2AuUjA/content .
  Added independent literal two-byte read timing regression. Explicit
  byte-clock timing remains an uncalibrated controller abstraction.
- All-targets tests002 passed213tests across69 result groups before that
  IIC timing fix. After the fix, excluded IIC crate tests003 pass22tests;
  strict workspace and IIC Clippy pass. Correct excluded-crate command:
  CARGO_INCREMENTAL=0 cargo test --offline
  --manifest-path simulator/backends/qemu/axi-iic/Cargo.toml
  --features host-std --lib.
  The earlier tests002 command omittedhost-std and failed compilation due
  to no_std unwind handling. Its failure log is retained.
- C ABI regression now checks real two-byte receive timing and values.
  Correct standard-library compile requires-DMD_SIM_HOST_STD when linking
  the combined qemu-core library. The first046 invocation omitted it and
  deliberately called the standard unwinder withNULL, causing a sanitizer
  crash in the test sentinel; it was an incorrect test build configuration.
  Corrected pic16-iic-ffi-046-002 andpic16-ffi-046 both passASan/UBSan.
  Existing no_std fatal-unwind assertion remains for its proper build.
- core-sources-046 pins the corrected library
  43f7c4c8a2fd3dde9daf8a0a734d958248afc10f72fbbabd3183759647f715d3,
  31352602bytes. Prior25a994...0451 remains verified in045CAS.
  Backend046UBSan is building with the same command, new output directory.
  qtest suite now also sets distinct nonzero pointers on a second PIC,
  checks the first pointer was preserved, rejects empty-FIFO artifacts,
  and requires successful reads from all three addresses. Its helper and
  main source hashes are in each assessment. Next runqtest002 then firmware.

### Firmware reaches the PIC application contract

- Backend046UBSan succeeds in184465ms. Binary
  4686ebb9d0ede06e5341b36b4904b5676127a37bede2c07f84063d0671a53cad,
  19567272bytes. All startup checks pass. pic16-qtest-002-ubsan,
  pic16-iic-regression-002-ubsan andpic16-s9io-regression-002-ubsan pass.
  The PIC test checks output-pin transitions and successful, fault-free
  read transactions at all three addresses. Workspace all-targets004
  passes213tests/69groups after the IIC fix; strictClippy004 andfmt004 pass.
- OriginalSD2022 run sd2022-pic-console-001 is a FAILED diagnostic:
  timed-out240064ms, no trace truncation, driverexit1. Its original ARM
  firmware reads flash, jumps to the app contract, reads version3, writes
  voltage code06, enables/disables output and sends real heartbeats on all
  three addresses. Each controller has30decodedcommands and197defined
  reads. It sends broadcast BM1387 read packets5405000019 on each S9IO
  link6/7/8. No ASIC is attached, so no serial replies are produced.
  Firmware then disables all three voltage outputs and stops heartbeats.
  The first watchdog expires atPIC56 and correctly raises unknown recovery
  code8; QEMU stops. The15s-delayed console script could not print its final
  service snapshot before this stop. No watchdog success or mining claim.
- Added Rust example s9_pic_audit. It bounds input reads, verifies every
  retained run input, checks originalSD/backend/report identities, compares
  live logs byte-for-byte with result streams, and records per-controller
  events/commands/power/errors. It always leaves board/mining gates incomplete.
  Two parser tests and strictClippy pass. Actual audit ofnative001 succeeds,
  verifying37inputs and writing its assessment.json. Supplying backend047
  for that046 run correctly fails with"backend evidence differs" and creates
  no report. The audit command is:
  target/release/examples/s9_pic_audit RUN BACKEND NEW_REPORT.
- Added absolute virtual nanoseconds to PIC C traces. Model library remains
  the exact tested43f7...15d3 from046. core-sources-047 retains the updated
  binding and references046CAS. Backend047UBSan succeeds in180609ms:
  32208aadd3c154a9fdd449070d598295741bb0730ae66c249b1fdd2c361cec72,
  19567368bytes. All six startup checks pass. pic16-qtest-003-ubsan passes
  against047, including output IRQs and bus trace checks.
- console-pic-check-002.sh samples actualservice/logs every2s for10s,
  starting when native GPIO904 appears. It also records Linux GPIO bank
  bases, widths and labels. This captures the firmware failure before
  unknown watchdog recovery, without changing any firmware timers.
- native002 invocation had a host path typo original-zynynq. It failed
  before execution; its assessment and driver log are retained. Corrected
  native003 is running against047 with the new console script.
  Current actual service logs show"Voltage controller application started",
  version0x03, voltage9.402579, and chain6/7/8 initialization. These are
  original ARM firmware messages, not synthetic boot lines. No PIC CPU
  execution or physical voltage calibration is claimed.
- duplicate-input-cleanup-010.json records60verified duplicate inputs,
  1143395114bytes removed from finished044/045 builds. Restore by copying
  from031CAS. Keep031canonicalCAS and all unique prior model libraries.
  All original archive files, backend binaries, logs and snapshots remain.
- Next: finishnative003, runRust audit, record the exact guest failure.
  Then connect powered/resettable BM1387 chains with real register and
  serial behavior. Existing bm1387.rs is only a tested packet/work codec.
  Archived driver lib.rs specifies63chips,114cores,25MHzoscillator,
  initialbaud115740 andtarget1562500. InitialASIC bit period is8640ns;
  newnativefirmware programs S9IOdivisor0x6b at200MHz, matching that period.
  Actualnative001 emits5405000019 on each link and receives no replies.
  Its GPIO output reset release values400,c00,e00 imply chain7/8/6 bits
  10/11/9 respectively; confirm againstnative003 bank/source evidence.
  Source gpio.rs has Rst(i)=888+i-1 andactive-low reset; it is reference
  evidence, not proof of a different board revision. Preserve exact profiles.

### New priority: ARM timer clock defect

- native003 diagnostic completed:131941ms, exit0, no truncation.
  Emulator CPU88.916934s user+1.387534s system, wall131.923s.
  Rust audit verifies37inputs and writesnative003/assessment.json.
  Native firmware logs three version0x03 messages and voltage9.402579.
  It correctly warns that BADCORE/FREQ calibration flash is invalid.
  The final sampled log precedes the later enumeration failure; the bus
  trace has all three real5405000019 requests, no replies, then all three
  voltage outputs disabled. Do not claim whole-board boot or mining.
- GPIO bank evidence now confirms outputbase884,width13,address41210000;
  inputbase897,width9,address41200000; Zynqbase906,width118. The archived
  Rst(i)=888+i-1 therefore maps chains6/7/8 to outputbits9/10/11.
- Crucial actual timing observation: native003 sends its nominal1s
  heartbeat everyabout3.38s of QEMU virtual time. ExamplePIC55 command16:
  118.68105,122.05961,125.44354,128.82164 seconds. Guest kernel says
  "sched_clock:64bits at333MHz" and selectsarm_global_timer.
  This is a real clock-model defect, NOT a reason to alter firmware
  watchdog delays. Fix it BEFORE attaching ASIC chains.
- Pinned QEMU source confirms BOTH a9gtimer andarm_mptimer hardcode a
  10ns tick (100MHz). a9mpcore has no clock input. Zynq SLCR only computes
  UART clocks; ARM_CLK_CTRL writes are stored without clock propagation.
  The firmware/kernel derive333MHz cpu_3or2x, causing the measured3.33ratio.
  Old diagnostics remain historical, not calibrated ordinary-timing passes.
- Existing qemu-clock-source-001/members/hw/cpu/a9mpcore.c and
  kernel-clock-source-001/members/drivers/clk/zynq/{clkc,pll}.c supply
  dependencies. New isolated qemu-timer-source-002 retains:
  a9gtimer.c ecf8e2a726945cdd3c41443c7d8d934e06e7299de92961a73568dc8217b347af;
  arm_mptimer.c6b44b8f3cec3f3300f1685cc9657b93737a45162ce25f7826094b493fd189ed5;
  cadence_ttc.c a35ec96362cd3e0289d2a2f2796866f2d1cbb29534f4f84ef41fc48b11309fbd.
  ParentQEMU archive remains2a40e670...76cce.
- qemu-timer-source-003 adds primary QEMU Clock/PTimer APIs:
  include/hw/core/clock.h8e43731157aaf5f019c3c5c16cf355076b1fab10d49a47aba9217f257ff99ac4;
  ptimer.h da9d9bb2dd9d660ea8f9a871b08479e570580f8e5bb533f310cd49ecf4fd4292;
  ptimer.c89df7917ffd63ff32c35131f8c6f1fc0c03ca31be552fe868ffb0a1653eedda7.
  Selected members copied only after hash verification. No source executed
  on host. ExistingRustqemu_s9_backend inspector gained these exact paths.
- Next implementation should REUSE QEMU timers and Clock API through C
  backend integration, not write another CPU/timer emulator.
  SLCR CPU source mux{ARM,ARM,DDR,IO}, bits4..5; divisorbits8..13;
  cpu_3or2x iscpu_div/2, gatebit25. KernelclocksCPU6or4x gate24,
  2x gate26 and1x gate27; only3or2x is needed forglobal/private timers.
  InputPS clock33333333Hz is declared by exactDTB. ProgrammedPLL40/div2
  gives333333330Hz peripheral clock.
- QEMU clock input supportsClockPreUpdate|ClockUpdate. Use these to
  synchronize counters before a frequency change, then propagate/reschedule.
  PTimer hasptimer_set_period_from_clock(timer,clock,prescale+1), preserving
  fractional periods; use it rather than integer-nanosecond rounding.
  Clock helpersclock_ns_to_ticks/clock_ticks_to_ns use32.32ns periods and
  saturate expiry durations. Handle disabled/gated clocks explicitly.
  Existinggtimer also computes advancing reads while disabled; its compare
  code comment says>= but code uses<. Review these when addingclock support.
- No clock patch has been written yet. Add a compiled qtest which programs
  SLCR, measures counters over exact virtual intervals, checks stopped
  clocks, prescalers, private-timer expiry, and dynamic frequency changes.
  Retain a failing047 baseline, implement, rebuild underUBSan, rerun tests,
  then repeat original firmware and compare heartbeat/guest timing.
  There are no active firmware/emulator runs now. Emergency shell70212
  remains. Agents remain unavailable due to the recorded account limit.

### CPU peripheral clock integration, build048

- Added `s9-cpu-timers.patch` against the eight pinned original QEMU files.
  `cpu-timer-derived-001` retains separate base/derived sources and parents.
  Reuses QEMU Clock, global timer and PTimer through their C interfaces.
  SLCR ARM PLL/mux/divider and gate25 drive cpu_3or2x, then A9 global,
  private and watchdog counters. No new CPU emulator.
- Fractional periods and reference-based comparator deadlines avoid
  rounding the333MHz clock to3ns. Disabled and gated timers hold count.
  Comparison now uses>=, with arbitrary positive auto-increment values.
  An explicit armed flag prevents expired one-shot timers from restarting
  after a clock change. Clock changes restart the fractional cycle phase;
  this is not physically calibrated. Migration formats changed and remain
  unvalidated. Whole-machine migration was already unsupported.
- Failing047baseline `cpu-timer-qtest-001-baseline` retained:100MHz
  instead of333333330Hz, disabled global counter still advances.
- Build048 underUBSan completed184021ms,exit0,no truncation; all patches,
  compiler and six machine startup checks passed. Binary19572232B,
  SHA-256930260cdaa548504a69f7763e95b38f82eb662151fe0853804293541388aff12.
  Compact reports inbackend-build-048-ubsan; fullresult/CAS in
  /tmp/md-sim-qemu-backend-048-ubsan. Rust model library remains43f7...15d3.
  core-sources-048 pins199files. The enlarged timer test was added after
  that build snapshot; its exact source and probe hash are in its report.
- `cpu-timer-qtest-002-ubsan` PASSES100transactions. Checks full333333330Hz,
  global disable/gate/prescaler, ARM divisor change, private one-shot expiry,
  arming while gated, rate change while running, expired prescaled one-shot,
  comparator increments3with thresholds at10and19ns, and watchdog counter.
  `pic16-qtest-004-ubsan` PASSES391responses with IRQ and PIC traces verified.
  Component passes only; no board/mining gate granted.
- `cpu-timer-clippy-001.log` passes strict affected example checks;
  `cpu-timer-fmt-001.log` passes. Post-fix firmware timing is being tested:
  `s9_boot_bundle boot work/simulator/original-zynq/sd-bundle-006
  work/simulator/qemu-s9/sd2022-pic-console-004 spl sd2022-v1
  backend=/tmp/md-sim-qemu-backend-048-ubsan budget=180
  presence-pin-fixture=e0 fan-fixture pic-contract
  console=work/simulator/qemu-s9/console-pic-check-003.sh`.
  Session52340. Script samples every1guestsecond from0through8 and has a
  final log sample. No firmware bytes or watchdog timeouts changed.
  Next: finish this run, audit exact inputs and heartbeat intervals.
  Then implement and connect powered/resettable BM1387 chains.

- native004 failed diagnosticwait50(exit1,83508ms), no PICcommands yet.
  native005 adds startup process/service logs and also failswait50(exit1,
  99197ms). At85.94guestseconds,dropbearkeyRSA is still running andS99
  service initialization is in progress. Dropbear's initial malformed/
  incomplete key startup attempts fail visibly before it starts later.
  These failed runs are audited and retained.
- native006 usesconsole-pic-check-005.sh(wait120,progress every20attempts,
  same8second final PIC observation). This is a longer diagnostic budget,
  not a relaxed boot/mining acceptance assertion. No firmware timeout was
  changed. It completes90616ms,exit0,no truncation. Audit verifies37inputs.
  Exact firmware really reaches all3PIC application contracts and then
  ASIC enumeration. Actual logs reportchain6/7/8"no chips detected".
- Measured PIC heartbeat intervals after clock fix:1.015484051 through
  1.056707642s over12intervals on55/56/57, including bus/scheduling overhead.
  Previous003range3.378100106 through3.404368682s. Rawcommandtimestamps in
  bothassessment.json reports; no synthetic heartbeat or watchdog success.
- Workspace checks afterclock changes:215tests,70groups,0failed/ignored;
  strictworkspaceClippy/fmt pass;361guide links,0broken. Logs inchecks002
  prefixedcpu-timer-workspace-tests-001,cpu-timer-workspace-clippy-001,
  cpu-timer-fmt-002,cpu-timer-guide-links-001.
- Started next real dependency: Rustserial8n1 event-timed serial contract.
  Distinct from exact archivedS9IO16-sample HDL implementation. It samples
  start/data/stop at bit midpoints, emits actual TXedges, has boundedqueue,
  framing/break/false-start/reset/rate/time errors. Nohostclock/hashcap.
  This sampling contract is an explicit ASIC adapter model, not calibrated
  electrical evidence. Both8640ns(initial) and640ns(programmed) bit periods
  come fromarchivedBM1387driver25MHz/8/divider.
  Initial4tests passed:all256RXbytes atbothrates,TXindependentwaveform,
  pollingphaseindependence,framing/break/reset/queue/time failures.
  Review changedreset to act atcurrenttimestampafteradvance, so it cannot
  silently skipelapsedwireevents. Re-runaffectedtests afterthissmallfix.
  NoASICchainbindingexistsyet; continuewithdecoder+poweredchipstate+QEMU.

### Powered BM1387 serial chain, build049

- Added bounded incremental packet decoder and explicit BM1387 chain state.
  Development profile:63chips,114cores/chip,25MHz,initial divisor26,
  reserved identity byte90 from archived driver test vector. No measured
  stepping claim. Address assignment, inactive state, ticket and misc/baud
  writes/readback, validated PLL writes, power/reset are modeled.
  Unknown registers, PLL lock/status and work execution remain explicit
  errors. No manufactured nonce, counter, PLL lock or work acceptance.
- Rustserial8n1 now drives a chain wire. Each elapsed serial deadline is
  processed before later pin changes; reply bytes have actual start/data/
  stop edges. Reset clears partial frames and queued replies and restores
  the initial rate. Queue1024 is a stated development resource bound.
  Zero added per-chip forwarding delay remains an uncalibrated inference.
- Added shared S9IO/ASIC timeline in md-sim-qemu-core. Historical UART
  edges stay inside Rust; QEMU cannot collapse them into a late GPIO
  callback timestamp. Same-time peers sample old outputs. C ABI adds
  explicit constructor, power/reset pins, error text and bounded trace.
  Existing S9IO without an ASIC peer retains its original interface.
- Tests:5packet,5chain,3wire,4serial pass; coupled S9IO tests2pass with
  identical complete traces for advancement batches9M,314159,257clocks.
  Tests independently decode63real wire replies and126FPGA FIFO words.
  Strict affected/workspaceClippy passes. Full workspace suite after these
  changes remains to run.
- C ABI tests use ASan+UBSan. First bm1387-ffi-049 FAILED because the test
  queued its command before the first enabled FPGA edge, which clears
  FIFOs per the archived HDL. Corrected test explicitly verifies that
  discard, then resends and verifies63replies. bm1387-ffi-049-002 and
  existing s9io-ffi-049 PASS. Failed test log retained. LeakSanitizer stays
  disabled due to the previously recorded ptrace environment limit.
- QEMU bindings connect PIC55/56/57 power outputs and GPIO bits9/10/11
  active-low reset to the three reference ASIC chains. New machine:
  md-sim-s9-bm1387-development. s9_boot_bundle accepts asic-contract only
  with pic-contract and records this separate development adaptation.
- Build049 completed180959ms,exit0,no truncation,UBSan. All patches,
  compiler and seven startup checks return0. Binary19604968B SHA-256
  445d708a35119be653f77c61bf2a45256111e8dc0e02c107162f66559702d010.
  Rust library31435396B SHA-256
  bfea3645a174144f7f4985ec396dc064df11cc4f0515fbd92bcd4e785816ea5a.
  core-sources-049 pins210files. Full build in
  /tmp/md-sim-qemu-backend-049-ubsan, compact reportbackend-build-049-ubsan.
- Compiled qtest now tests three powered/resettable chains through real
  MMIO, IIC, GPIO and serial FIFOs. Its helper hashes are pinned separately
  because it was added after the model source freeze. Running
  bm1387-qtest-001-ubsan(session38303), with explicit30s wall/20s CPU
  component budget. Next: inspect exact responses/traces, fix failures,
  rerun timer/PIC regressions, then execute actual archived SD2022 firmware
  with the new chains and implement its next real missing operation.
- Release stays incomplete: frozen matrix79,599rows,zero passing exact
  physical targets. Agents remain unavailable until the recorded external
  account limit resets; independent review is not passed.

- bm1387-qtest-001-ubsan PASSES859responses and exact byte/command traces
  on all3chains. Power loss onchain6and reset onchain7 independently stop
  replies whilechain8continues. This test uses the QEMU test clock.
- Actual archived firmware run sd2022-bm1387-console-001 reaches original
  U-Boot, then fails at~500ms: S9IO link6/7/8 ASIC pin phase outside core
  period. Investigation found md_asic_pin read QEMU time twice. Under TCG
  those reads advance, unlike the frozen qtest clock. The second read can
  exceed the residual0..4ns phase. No guest timeout or assertion changed.
- Fixed C binding: md_advance_to and pin update use one captured timestamp.
  Build050 running(session12157), same Rust library as049. Source freeze
  core-sources-050 pins211files. BM1387 contract documentation changed
  after freeze to record test and failure; the frozen copy remains exact.
- Timer/PIC regression attempts003/005 failed report finalization because
  concurrent cargo test replaced their executable. The running qtest still
  used its old inode, but current_exe returned a deleted pathname. Fixed
  evidence hashing to read /proc/self/exe, which identifies the executing
  inode. No failed report counted as a pass. Repeats cpu-timer004 and
  pic16006 PASS100and389expectedresponses respectively(PIC also2IRQevents).
- Workspace all-targets check PASSES229tests in74groups,0failed/ignored;
  bm1387-workspace-tests-002.log. Earlier release workspace check passed
  200tests/47groups but omitted example tests; it is not the full check.
  Strict workspace all-targets Clippy passes bm1387-workspace-clippy-002.
- duplicate-input-cleanup-012.json records30verified duplicate049CAS
  objects,571697557B,removed against preserved031CAS. Keep049unique
  bfea...5a Rust library. Restore via copies, never hardlinks.

- Build050 completed181668ms,exit0,no truncation,UBSan; seven startup
  checks pass. Binary19605032B SHA-256
  1877124b488f14d902e80fce5b48d0f295cafcc67b208bc4fc1f8698807950f4.
  Compactreportbackend-build-050-ubsan. bm1387-qtest-002-ubsan passes859
  responses/ASICtraces. duplicate-input-cleanup-013 removes30verified
  duplicate050CASobjects571697557B against preserved031CAS.
- sd2022-bm1387-console-001 ends at240s deadline after the phase error;
  audited withs9_pic_audit. sd2022-bm1387-console-002 (050defaultclock)
  passes that defect, executes actual U-Boot/Linux/BOSminer initialization,
  then stops at110.277virtualseconds on PIC57 UnsupportedCommand(aa).
  All inputs and failed output are audited. Contrary to an early progress
  description, no ASIC bytes/commands occur: only ASICpower/reset events.
  PIC57 receives55,aa at109.153/109.168s, then anotheraa at110.277s.
  PIC56 has a1.019s gap between enable opcode15and its argument01.
  Investigate IIC/guest scheduling; do not ignore unknown commands or
  pretend ASIC enumeration passed. The exact failed sequence is retained.
- Added optional s9_boot_bundle icount-shift=0..6 diagnostic. It records
  2^shift virtual nanoseconds per guest instruction, alignoff/sleepoff,
  unchanged device periods and explicit absence of physical CPU calibration
  or whole-board deterministic replay claims. Original firmware untouched.
  Also corrected adaptation metadata to mark attached ASIC peers and to
  distinguish the explicit presence-pin stimulus from physical detection.
- sd2022-bm1387-console-003 usesicount-shift1. It FAILS110168ms,exit1,
  no truncation; emulator SIGKILL at CPUlimit(109173805ususer+935542ussystem).
  Last guest outputfailsafe at3.430747s. This is a resource failure, not a
  boot pass. Slower declared guest instruction clock shift6 now being
  tested inconsole004(session42937), same backend050 and budget240/CPU110.
  No limits weakened. Its latest output reachesfailsafe at83.925352guestsec.
- qemu_s9_backend reuses existing isolated Rust inspector for two exact
  vendor archives. bmminer-mix47191d...fc5a2,commit7aebcf2c2cb7bd8cafca7f920d30011d0547d16a;
  single-board-test624b75...c8db,commitb5c89be17a1f81734702ca89478154f57101f44a.
  references.json and verified members saved inbm1387-vendor-source-001
  andbm1387-vendor-test-source-001. These establish existing opencore
  generation and register14hash-counting writes; they do not establish
  missing PLL lock timing. Public discovery used Bitmain's own GitHub
  repositories, then the archived hashes, not current HEAD compatibility.
- Added BM1387 nonce Layout dependency for real work search. It follows
  archived CoreAddress bits7:2chip,30:24core, and all19remaining bits.
  Round-robin chip/core traversal is explicitly simulator scheduling.
  Three tests pass, including all524288nonces for one core with no
  duplicates,63x114ownership and absent chip/core holes. It is not yet
  connected to hashing/firmware and grants no mining gate. Rustmodel
  library staged in050 remains the prior bfea...5a build; do not claim
  this new nonce module is in that pinned backend.
- Latest affected clock/inspector Clippy passes;3boot-runner tests pass.
  Guide check362links0broken. Full229-test result predates nonce module;
  run affected checks after further fixes and full suite before release.

### TTC zero-time callback loop

- Both instruction-count runs003/004 fail at the failsafe prompt with the
  same110sCPUlimit.004uses64ns/instruction and reaches83.925352guestsec,
  then emits no more guest output. Both failed runs now have input audits.
- Inspected pinned upstream cadence_ttc.c SHA-256
  a35ec96362cd3e0289d2a2f2796866f2d1cbb29534f4f84ef41fc48b11309fbd.
  cadence_timer_get_ns floors fractional16-bit tick deadlines. At a
  fractional remainder below1ns it schedules at the current timestamp;
  cadence_timer_sync sees unchanged time and cannot advance the counter.
  This can spin forever under qtest/icount while realtime hides the loop
  by eventually advancing host time. No new CPU emulator is involved.
- Added a literal TTC register regression to existing CPU-timer qtest:
  upstream133MHz,2ticks=15.0376ns, no event at15ns, interval event at16ns.
  Old050backend FAILS ttc-qtest-001-baseline:106of112responses,exit137
  from CPUlimit. It hangs atclock_step15 exactly as predicted.
- Derived patch s9-ttc-deadline.patch uses one denominator and ceiling
  division without overflow. Only callback deadline rounding changes.
  Retains existing upstream133MHz input, counter precision and clock-tree
  limits. Base/derivedfiles+parenthash inqemu-s9/ttc-derived-001.
  PatchSHA2ceca97c4d24ebd59f07db27ef5dda440027214f509f77b299826a51ae1bf7e3;
  derivedC766c8dd77f6ad4409c7512d00a67a0d275a8c6bdada2a5caeb42b344be382f78.
- Build051 nowrunning(session82277), UBSan. Sourcefreeze214files in
  core-sources-051. NewRustlibrary31438704B SHA
  f4530a59b68e3e0415f82934339a353b7c20cf6ddc5f4e194f0249a7943e795a;
  priorbfea...5a preserved049CAS. NonceLayout nowusescandidate_count()
  instead of unnecessarylen/is_empty;3affectedtests stillpass.
- Workspace all-targets tests/session83817 and Clippy/session59152 running.
  Next: inspect051build; run112-response timer test plus859-response ASIC
  regression; original firmware withicount-shift1; diagnose actual next
  unsupported operation. Do not relabel prior failed firmware as passed.

- Build051 completed185066ms,exit0,no truncation; patches and seven
  startupchecks pass. Binary19605896B SHA-256
  712a10162fae03c94cf139a110b050a221c7d362bc61e26d843d13d78301bb70.
  Compactreportsbackend-build-051-ubsan. ASICqtest003 passes859responses.
- TTCqtest002 remains FAILED:all112responses exactly match, but the process
  consumes5CPUseconds after the input closes and exits137, not the expected
  idle wall deadline. TTC disable leaves a callback scheduled. Added
  timer_del when disabled; do not weaken the termination assertion.
  Newderived002 preserves prior001exact bytes(766c...2f78). Newpatch
  6131e89dece0ed26877f269f17af3d343499c28074ef0b36e127f4b32f5d521b,
  derivedC6b80ee9036073c5cb555551d3fdfa1cee3de701727c01e17f6cb3efca2608c76.
- Workspace232tests/75groups PASS,0failed/ignored; strictClippy/fmt and
  Bitcoin fixture pass. Added explicit2^32 candidate-count boundary to
  nonce test after that run; affected3tests pass again in
  bm1387-nonce-tests-003. Sourcefreeze052 includes this final test.
- Build052 preparation FAILED before process-spec creation due disk quota;
  no backend build or startup was performed. A new tool shell also failed
  to register/tmp/.git due quota. Used existing emergency shell70212 to
  remove verified051duplicates: journal014,30objects571697557B→031CAS.
- Freed project-owned rebuildabletarget/debug/examples cache; journal
  cargo-cache-cleanup-002 records286files,1395253248physical bytes by
  unique inode. Release binaries, source snapshots, CAS and reports kept.
  Failed052duplicates journal015 removes25objects149320352B→031CAS.
  No original archive or unrelated project removed.
- Build053 nowrunning(session79640), same exactsourcefreeze052(214files)
  andf453...795a Rust library. This is a retry after freeing storage, not
  an unchanged external failure. Next: compiled112response timer test,
  ASICregression ifneeded, then original firmwareicount-shift1.
  The release stays incomplete. No exact physical target/mining pass.

- Build053 also FAILED during input preparation, before process-spec or
  compiler execution. Its incoming Rust library copy stopped at15556608B.
  Diagnosis: Cargo cache is on the repository/root filesystem, whereas
  build CAS copies are on/tmp. Root-cache cleanup did not free/tmp quota.
  Failed052incomingtoolchain copy294830080B remains; expected full031
  objectcc98...aa5 is295943185B. Do not treat incomplete incoming objects
  as valid inputs or count either failed build as executed.
- Freed actual/tmpspace using duplicate-overlay-cleanup-001.json:
  26identical derivedoverlay.tar.gz copies,3286208484B, SHA
  147a38c11432035d5a9132f1a65171e808b2c7bad1769986da0e38d951d40390,
  each126392634B. Canonical/tmp/md-sim-qemu-backend-031/overlay.tar.gz and
  its031CAS copy are preserved. Restore copies from that canonical path.
  Original downloads, binaries, all build reports and manifests remain.
  Measured/tmpusage falls38G→35G. No unrelated project was removed.
- Build054 nowrunning(session22058), same sourcefreeze052 andf453...795a
  library. This follows a material storage fix. No other build/guest runs
  are active. Emergency shell70212 remains available. Next: finish054,
  run112-response timerregression and originalicount firmware path.

- Build054 completed185851ms,exit0,no truncation. Its UBSan binary is
  19605224B, SHA-256
  ebf631d5e1363a5703ccb15355fa47a1ee96502f4f6e5368aa7086ed7ac58b10.
  Compact reports saved under backend-build-054-ubsan.
- TTCqtest003 PASSES112responses and expected idle wall timeout. Unlike
  051 it does not spin into CPU-limit exit137 after input closes.
  Original firmware sd2022-bm1387-console-005 is running with icount-shift1
  (session31879); ASIC859-response regression004 also running(session40793).
  No exact miner target or release gate has been granted.

- ASICqtest004 PASSES859responses with054. Original005icount1 FAILED at
 110183ms, emulatorCPU~110.136s, before enumeration. The TTC stall is gone:
 actual kernel/services execute and all three PICs reach app/voltage/enable
 with197definedreads each, no undefined reads or protocol errors. Audit saved.
- Original006icount6 FAILED36639ms due diagnostic `logread` before logd exists,
 not a backend error. Audit saved. New console-asic-check-006 guards optional
 startup log collection, waits for actual bosminer PID and gpio, then
 observes30seconds. No boot/mining acceptance threshold is changed.
 Original007 with icount3 now running(session80021).
- Source review found a correctness defect in the unused nonce layout:
 archived lib.rs explicitly says114core addresses are non-consecutive.
 SHA1eaa099131265a837c3e3ebbe03674fe22193206573a69fa87e08c035d522a6e.
 Replaced core-count inference with explicit per-chip core maps. Missing
 ranges remain holes, including different maps on different chips. Added
 sparse/non-consecutive and invalid/duplicate core tests. All4tests pass
 in bm1387-nonce-tests-004. Consecutive-map tests now clearly say synthetic.
 No compiled054input changed: its frozen sources and library are retained.
 The exact physical core map remains unresolved, not incompatible.

- Original007icount3 FAILED at emulator CPU limit, wall105304ms. It reaches
 actual ASIC query0x54 onchain6 at140855231545ns and starts63real serial
 register replies; chain7 also starts its query. Audit retained. Still no
 enumeration/mining acceptance. No firmware bytes or device clocks changed.
- Performance diagnosis: active UART made idle_limit return0 on each200MHz
 FPGA clock. QEMU scheduled one callback per5ns. Implemented safe divider
 skips before the next registered sample, requiring stable RX synchronizer,
 stable TXD, empty RX FIFO and no pending sample or divisor change. Receive
 packet state Read(index) may also wait when UART FIFO is empty.
- First newtests001 FAILED two assertions: active skip benefit was too small
 because Read(index) was omitted, and BREAK recovery stimulus gave only3
 high bits while a frame could still be in progress. Fixed Read wait and
 gave12highbits to finish the in-flight frame. No output assertion removed.
 Pair checker now requires unchanged TXD level (including low), not always
 high. All skipped cycles remain compared with ordinary execution.
- tests002 PASSES24tests (AXI6,device3,idle10,UART5). Active115kbaud transfer
 skips>90% of clocks with identical outputs. Release coupled-chain tests
 PASS2 including every timed edge for63replies at three chunk sizes.
 Strict nonce Clippy and362guide-links pass. Next: build055with exactsource
 freeze, compare compiled859response ASIC traces to054, timer regression,
 then rerun original firmware. Agent slots remain usage-limit errored.

- Sourcefreeze055 pins214files. Rust library31436018B SHA
  d2f94ed7e3e9b974f93623ae5da55ccb93a6868206632403375eb04aa3fbc7f2.
  Build055 PASSES180101ms exit0, no truncation, all patches/startup checks.
  Backend19605736B SHA
  13e94df025329c5255ef3d2529a6de2151d3caddeebdec216ec3d08dea5129eb.
  Compactreports backend-build-055-ubsan. BM1387/S9IO C ABI ASan+UBSan
  PASS (detect_leaks=0 due existing tracing limitation). Clippy/fmt pass.
  ASICqtest005/session44975 andTTCqtest004/session31634 running.
- Extended source inspector for main.c/miner_type.h, then hash-checked
  isolated inspection bm1387-vendor-test-source-002. Vendor main.c SHA
  59dc81a68c487ae791623f1d28c6cb19224f01426453b609191de7e8585e1161,
  driver-bitmain.h9d33c33175f39c7c306128445b6ff14915a135a5d9270c480596b8cb11630555.
  Vendor receive maps rawcore0..56 directly,64..127 minus7; header defines
  ASIC_TYPE1387/114cores. Inverting114counted slots gives0..56,64..120.
  Recorded source-derivedreference JSON with hashes and explicitlimits.
  Archive miner_type selectsT9_18, header alsoS9_63; no whole-board
  compatibility or exact silicon claim. Extra5th nonce test PASSES.
  Inspector/test/JSON additions after055freeze do not affect its model.
  Public primary-source search found no further validated exact map.

- 055ASICqtest005 andTTCqtest004 PASS859/112responses with expected wall
  termination. uart-skip-compiled-comparison-001.json verifies all16191
  ASIC+S9IO eventlines exactly match054, including timestamps and order.
- Original008/055icount3 FAILED CPUlimit at105104ms wall. All three actual
  guest queries return63chip replies. Chain6 completes63address writes,
  PLL006d0511, Misc00208180, ticket00000000 and63filter readbacks.
  Chain8 completes30address writes;chain7 four before termination. This
  is observed firmware traffic, not injected synthetic messages. Audit saved.
- Added second optimization: when UART FIFO is full and CRCs idle, TX
  byte-emitting states cannot change until the UART consumes a byte.
  Skip only up to the UART's earlier safe sample boundary; still advance
  the24-bit work timer. Enabled state and bus/reset rules remain intact.
  New150B archived work+two queuedcommands test checks every ordinary
  clock, literal bytes and>90% skippedclocks. 20affectedtests PASS003;
  fullfirmware122tests/22groups PASS004; coupledcore2tests PASS004.
- Freed finished054/055CAS duplicates via journal016:
  60objects1143395114B→031CAS, unique libraries/reports/binaries kept.
  Next: sourcefreeze056, buildUBSan056, compiledtracecomparison/regressions,
  original firmware009. No exacttarget mining or complete release pass.

- Sourcefreeze056 pins215files, including the source-derived core-map JSON.
  Rust library31435838B SHA
  d761959a05568dcbbe7e9d8a568302e171692f35f30b8f7826c4a1b2beee818d.
  Preserve prior d2f94...fbc7f2 in055CAS. Build056 running session37652,
  output/tmp/md-sim-qemu-backend-056-ubsan, driverlog
  checks-2026-10-20-002/uart-fifo-backend-build-056-driver.log.
  Both056C ABI ASan+UBSan tests PASS (leak check still unavailable).
  StrictClippy002/fmt002/guide-links003 PASS. No active guest remains.
  Next: finish056, ASICqtest006, TTCqtest005, exact event comparison with055,
  then original009 using console-asic-check-006.sh andicount-shift3.

- Build056 PASSES181909ms, all22steps and startup checks. Backend SHA
  9227116da943fcb7d7c101b2855998120ebdbcdc6feb49ece3f70339589c6e7a,
  19604552B. ASICqtest006 PASSES859responses; TTCqtest005 PASSES112.
  Compiled comparison002 matches all16191ASIC/S9IO events against055.
  Added bm1387-burst probe:1058responses, queued63address writes while
  UART FIFO fills. Both055/056 PASS; comparison003 matches all23192
  eventlines exactly, including order and virtual timestamps.
- Original009/056icount3 reaches all3chains'71setupcommands and an actual
  150B work packet onlink7 at144782233555ns(lastTXbyte started).
  Decoder/checksum audit identifies4zero midstates and tail
  ffffffff00000000ffffffff, id0. Backend stops explicitly on
  Chain(WorkExecutionUnimplemented), then outer240065ms wall timeout.
  PICaudit verifies37inputs,197definedreads percontroller, no protocol
  errors. Full original-board and mining gates remain incomplete.
  Retained asic-packets.json generated by new Rust s9_asic_audit example;
  it grants no acceptance. No unchanged external agent failure retried.
- New Search cursor tests PASS3:128fresh full-header oracle comparisons,
  sparse nonce ownership, shared10kcap, fixed genuine filtered response,
  invalid inputs, exhausted/early polls. Clippy001 found3test-style errors;
  fixed ranges/reference slices, Clippy002 PASS.
- Work integration in progress: explicit core maps, startup gate state,
  per-chip ticket masks, job replacement and shared run-owned rate budget.
  Search response encoding now avoids recomputing a second uncounted hash.
  Wire schedules hashing separately from UART; only computed filter matches
  enter the serial queue. New QEMU ABI shares one budget across3chains.
  Register/search/nonce/wire regression001 PASS16tests; strictClippy001 PASS.
  QEMU machine patch and new integration tests are still being completed.
  These source changes are NOT in frozen backend056. No new firmware pass.

- Completed Rust work integration. Shared Budget uses wall time, one
  attempt per grant, weights by enabled cores, and no idle/reset credits.
  Virtual polling is separate. Native fixed32+per-chip ticket filter is
  unchanged. No filter override. Whole-path fresh FPGA tests independently
  reconstruct full headers for every attempted nonce on two fresh jobs.
  Core tests002 PASS4; firmware tests002 PASS129/24groups; strictClippy003/004,
  fmt001 and guide-links002 PASS. C ABI BM1387/S9IO ASan+UBSan057 PASS.
- Added reproducible checkpoint_qemu_core helper. Freeze057 retains285
  source files, checks before/after build, and library31691448B SHA
  7192ac14313860385185569a37c0be9486d7983a69ed13cfb9afeab3327075ae.
  This differs from056;057Ctests use that exact retained library.
  reclaim_qemu_inputs journal017 safely removed60verifiedduplicate
  053/056CAS inputs1143395114B; restore sources remain031CAS. Unique
  libraries and partial transfers retained. /tmp about35GiB afterward.
- Build057 FAILED27309ms in C compilation: unknown ZynqMachineState.
  GNU patch fuzz inserted our new type atline45, before the parent header.
  All failure inputs/reports retained. Strict attempt003 exposed other
  insufficient-context hunks. Regenerated the complete patch from the
  retained exact parent and edited derived C source. New strict004 applies
  without fuzz or offsets and byte-compares equal to the intended output.
  Parent5817bac9f65361e0cfc49b9fc3d3629d1e5b12582f22718191ccb3d608606e29;
  deriveddb77ee1ed578a98bd538405faf0683c96f847506e8af7297fe9f86742327f5c8;
  patch90d91d040048c9ff71b28576f70cb9f427f94a82814cd009f8cbf8a03824dc08.
  Build recipe now rejects fuzz and checks both parent/derived ARM hashes.
  Next: freeze058/build058, compiled ASIC/TTC regression and actual
  firmware010. Also extend ASIC audit to independently verify nonce traces.
  No original-board, exact-target mining or release completion gate passes.

- Build058 PASSES185631ms,23steps and8startup checks, including0.1H/s.
  Binary d4ff95be487aa422b34b324ad755164acb81970d04014f906fa57506dd705d8f,
  19806648B; library7192ac...d8f from285-file freeze058 matches057.
  Current strict BM patch hash rechecked90d91d...dc08; prior entry correct.
  Compiled burst003 PASS1058responses; TTC006 PASS112responses.
  All23192ASIC/S9IO eventlines match056, including timestamp/order.
  Firmware tests002 PASS129/24groups, core tests002 PASS4, C ABI
  ASan+UBSan057 PASS; strictClippy003/004/005,fmt001,guide-links002 PASS.
- Original010 uses058 andicount3. It FAILED wall timeout240062ms,
  without truncation. Firmware explicitly disabled voltage around145.9s.
  PIC watchdog expired10s after its last heartbeat and stopped the VM.
  No jobs or computed hashes. Packet/PIC audits retained. Hash audit
  correctly rejected this run for no attempts.
- Original011 uses058 anddefault clock. It FAILED wall timeout and32MiB
  trace truncation. Archived bosminer sends actual startup work. Independent
  bitcoin_hashes audit checks3539computed nonce traces from158jobs in the
  retained prefix against active midstates, nonce ownership and ticket mask.
  Report: qemu-s9/sd2022-bm1387-console-011/asic-hashes.json.
  These are null/core-startup packets, not fresh pool jobs or accepted shares.
  Guest enters preheat with unknown temperature and explicitly disables power.
  No exact-board, pool or regtest acceptance follows from this diagnostic.
- 2026-10-21: all3review agents remain usage-blocked untilOct25 08:48AM.
  No unchanged retry. Independent review gate remains incomplete.
  Added optional asic-events diagnostic to omit repetitive S9IO MMIO
  reads/writes while retaining serial/state events and other device traces.
  Exact selection is recorded. Driver build001 PASS. Next original012
  uses058 andconsole007 streaming logread to expose shutdown cause.
  UART oversample-counter skip under development; clock-equivalence
  regressions running. This change is not present in058.

- UART oversample skip now advances only stable counters up to the next
  TX bit transition or RX sample point. All pins, synchronizers, FIFO
  actions and bit periods remain clocked. Tests001 PASS19; tests002 PASS24
  including fresh mining and independent audit. Core tests003 PASS4.
  Clippy001 andguide-links001 PASS. Freeze059 retains286files and
  libraryba1486a7e1ce8e164559c823b397e735a9d8c785042998bffe296b3069c7af0f,
  31697464B. Build059 PASS184765ms; binary45089de12eb738ffa872eb37e2cfcf3817942efce9208dcb1185c9d0150bc215,
  19806424B. Burst004 PASS; all23192events exactly match058.
- Original012/058 streams logread and omits S9IO MMIO diagnostics.
  FAILED240049ms with16MiB stderr truncation (32MiB aggregate trace budget).
  Audit982hashes/181jobs in retained prefix, chains7/8; no native-filtermatch.
  Original013/059 increases diagnostic aggregate trace budget to64MiB.
  FAILED240033ms without truncation: firmware explicitly disables power,
  PIC0x57 watchdog expires at128822393211ns, model stops on state8.
  Complete-log hash audit running43970; report retained when done.
  New audit schema includes execution-result hash/status and trace-truncation
  flag. Original011 v2 report explicitly marks prefix-only evidence.
- Source inspection braiins-monitor-source005 adds halt/monitor/sensor
  source from unchanged dc5f587...3a363 archive. It is legacy reference
  evidence, not proof of proprietary2022 source identity. Unknowntemperature
  alone does not trigger legacy monitor shutdown; no shutdownreason observed.
  Real firmware currently has no reachable pool. Investigating idle stop.
- Private network work: inherited socketpair exists, but QEMU SO_TYPE
  query is denied; socket backend and socket chardev fail. Stream backend
  starts. Probe003 retains failures. Probe004 adds private named pipes;
  QEMU guestfwd pipe backend starts successfully. No guest packets yet.
  Added serve_pipes to existing Rust Stratum verifier, with bounded POSIX
  polling. Separate anonymous-pipe subscription/work/cancellation test
  PASS; all4Stratum tests PASS. Firmware itself will use TCP via libslirp.
- Build060 enables pinned installed libslirp4.9.5 (Arch4.9.5-1).
  Library91ebce3a6429ccb3bf5331fdb06360ad2229d28c59cf0d43ae48e251535b4068,
  148656B; headers/pkg-config hashes retained in build report and closed CAS.
  Source build of installed library is not reproduced. No new TCP stack.
  Freeze060 library matches059. Build060 PASS184401ms, no truncation,
  including private-network startup check. Binary
  7c1e041177a615d0322727035cafaa92391fa6b39c7f8bc1afd905859971e87c,
  19841816B. Compact reports qemu-s9/backend-build-060-ubsan.
- Added boot local-pool diagnostic. QEMU restrict=on,IPv6off, one guest TCP
  forward10.0.2.100:3333 to run-owned FIFOs; no listening host socket.
  Actual existing pool verifier generates fresh seed/time and low difficulty,
  retains jobs/submissions/protocol, and stops with the guest diagnostic.
  Native ASIC filter remains unchanged. Full-board/regtest gates incomplete.
  Initial lib/config extraction004/005 correctly failed: files absent from
  original ramdisk. Exact init script006 shows bosminer.toml is generated at
  START99. Sourcehash4f13a361f3ccc1ba40d592fd8e90b8fde490311dc2db27ab19c728c9e337c215,
  5828B. Script008 installs explicit private-pool configuration beforeSTART99;
  it retains default autotuning and checks miner has not started.
  Original014 RUNNING session47594, backend060,console-private-pool-008.sh.
  Driverlog checks-2026-10-21-001/original-pool-014-driver.log.
  Private-pool boot driver build001 PASS. Follow-up Clippy pending.
- Cleanup019 verified/deleted60duplicate input objects1143395114B from
  finished046/058 stores; restore by COPY from031CAS per journal.
  Initial cleanup call omitted/inputs and was safely rejected before mutation.
  Keep046unique library and0587192library. /tmp remains about36GiB.
  No other live guests or builds besides014. Agent usage blocker unchanged.
  Next: inspect014network/config/actualwork; audit013; fix actual failure;
  add compact packet traces if current event volume exceeds diagnostic bound.

- Continued private pool path: original014 fails early because network.lan
  is not created yet. Console009 waits for that original guest config.
  Original015 has real guest TCP and mining.configure, but old pool declines
  version rolling. QEMU then exhausts CPU110s. Added BIP310 negotiation,
  fixed-mask sixth-parameter submission and independent header checks.
  Archived BIP310/BIP320 identities retained in bip310-source-002.
  Tests bip310-tests001 PASS7mininglib+5Stratum; library-tests002
  PASS30firmwarelib+7mininglib; strictClippy004 PASS. No share claim.
- Original016 negotiates BIP310, subscribes, authorizes and receives fresh
  notify. It exhausts CPU110s, with0submissions. New mining-board profile
  bounds CPU240s/address3GiB/file64MiB; driver sets CPU=wall-10.
  Pool seed/time/deadline are now frozen in CAS input before launch.
  Pool thread exports its result before outer deadline even if VMpaused.
- Original017/060 FAILED240s timeout, complete trace. It negotiates actual
  guest configure/subscribe/authorize and receives first fresh job; no
  submissions. Model independently audits11881computed hashes from228
  startup jobs on6/7;0nativefiltermatches. All tails are null startup work.
  Chain8failsPICversionread; firmware stops6/7, thenPIC57watchdogexpires
  at110066326278ns. Original board/mining/regtest remain incomplete.
- Exact cause found in017 backend.log lines37618ff: PIC57 accepts55 at
  100179503519ns. Guest ISR read precedes STOP, ISR toggle follows STOP.
  The model clears NOT_BUSY even though the bus is idle. Guest times out
  and retries55 at101300593687ns, thenaa/17; parser correctly rejects
  the55,55,aa prefix. ReadNACK is a consequence, not the original cause.
- AXI IIC fix implements PG090 persistent idle interrupt and reserves
  busy state for enabled START during address interval. No bus callback
  or ACK is manufactured. Model remains a byte-clock abstraction.
  Tests exercise all read/clear completion interleavings and disabled
  queuedSTART. iic-bnb-tests002 PASS24; tests001 failed to compile because
  host-std was omitted (retained, corrected command useshost-std).
  Added compiled PIC regression at ACKed55/versionread. Build pending.
  New note simulator/backends/qemu/AXI-IIC.md records onlinePG090basis
  and explicit missing full source capture. No unchanged agent retry;
  three agents remain accountusage-blocked untilOct25 08:48AM.

- Freeze061 retains291files, library
  decbdcba9ad37e5bca11d0344a8a064144096ff732865bfb2b0e488016bca207,
  31788456B. Build061 PASS, exit0, no trace truncation. Binary
  8dbce31ee6d25a94b0098ba70f0f12adccc2756d6ef9a94263f77ad5f1373f88,
  19837048B. Compact reports backend-build-061-ubsan.
  iic-bnb-controller-qtest001 andiic-bnb-pic-qtest001 PASS throughactual
  compiledQEMU; latter checks STOP betweenstatusread/toggle and exact
  subsequentPICversionread. Correct queuedSTART statusbusy expected44.
  Rustcore integrationtests002 PASS4; core --lib001 has0tests, notcounted.
  IIC CABI ASan+UBSan061 PASS (detect_leaks0 sameptracelimit).
  StrictIIC/firmwareClippy,fmt009 andguide-links004 PASS.
- Cleanup020 reclaims60verifiedduplicateCASobjects1143395114B from
  finished059/060 inputs. RestoreCOPYfrom031CAS perjournal. Unique060
  libslirp/buildrecipe and eachbackendbinary remain. Archiveuntouched.
- Original018 RUNNING session22942 with061,defaultclock,240sbudget,
  sameconsole009,localpool,63x114referencechainsx3,hashcap10000,
  originalSD2022bundle006. Driverlog original-pool-018-driver.log.
  CompiledBM1387burstregression RUNNING97212. Next inspect018actual
  firmware and nextdeviceerror; do notinferminingfromsubscription/jobs.

- Build061 wall186048ms. CompiledBM1387burst PASS; exact23192ASIC/S9IO
  events match059burst004, includingvirtualtime/order. Comparison001.
  Firstcomparecall hadwrongbaselinebasename and failed beforewriting;
  corrected path bm1387-burst-qtest-004-ubsan.
- Original018 FAILED240044ms, notruncation. All3powercontrollers/chains
  initialize; noIICfaults/NACKs. This verifies the bus-idle fix in firmware.
  Firmware stilldisablesvoltage about2safterpreheatstarts. PIC56expires
  at108656704265ns. Audit16203hashes/342startupjobs, nofiltermatches.
  Pool0submissions; noexactboard/miningpass.
- Original019 usesconsole010 to add only RUST_LOG=debug toverified
  init script4f13a361...c215 onprivateoverlay. Readbackadaptedinit
  54579c43eefb210b6d023d8db3d3c800e456bf7efcf4b7762074a26243a3c0d6.
  Firmware reports debug compiledout bymax_level_info. This external
  binary limit cannot be fixed by another logging setting. RunFAILED
  timeout/no trace truncation; samepreheatshutdown,noacceptedshare.
- Actual019serialecho consumedliteralTAB asinteractivecompletion.
  Fixed serial_console::script_frame withprintableASCII POSIXprintf
  reconstruction andeval. Sentinelpreservestrailingnewlines; chunked
  argumentsboundinputline lengths. Noexternaldecoder/tempfile.
  Testsserial-framing001 PASS5, includingquotedcontrolbytes, UTF8,
  longinput,exitstatusandnotexecutinguntilentireframearrives.
  StrictClippy001,fmt010,driverbuild001PASS.
- Original020 RUNNING23691 withsamebackend061 andnewhostserialdriver.
  Console011 comparesliteralTABagainstprintf-octalinsideguest, then
  configuresprivatepoolandpollsnative4028summary/devs/notify/pools plus
  procdservicestate. NoRUST_LOGpatch. Guestnetwork/clock/hashcapunchanged.
  Driverlogoriginal-pool-020-driver.log. NextinspectactualAPIreason,
  verifyliteralTABmarkerandrecordexactresponse. Allotherguestsfinished.

- Original020 failed at the240s bound. The literal TAB check passes inside
  the archived guest. Its BusyBox nc has no -w option, so those API probes
  did not execute. Native procd retains the same miner PID across the
  earlier shutdown. Do not count a native API pass.
- Exact isolated archive extractions retained in native-s9: boser-init008
  d4b3077008b50414eb48d22045400411c4f2c36dcece42271de36086b848fdbd;
  bosminer-template009
  1b3b443c70ab43a6700c228ebb5baec1f23d559ad1db1d5aaeb07dec1fcb05a1;
  bosminer-monitor010
  5dd1adc64ed54033c2b9ed6c1998de31c744d133fbb4bfc77f11bef30b3f2a1f;
  boser-app011
  1c318483df06a03e460e35f7419d8562c007f5dc4965a20d412c96392ad36423.
  The original template supports autotuning.enabled=false. The monitor
  uses4028 for queries and4029 for LED control. It does not stop power.
  The original boser UI uses /graphql and WebSocket /graphql.
- Original021 uses console012, supported fixed-frequency configuration
  (autotuning disabled), and nc children bounded by own PID. It continues
  past preheat and receives actual new work. FAILED240s; trace truncated.
  Prefix audit:358jobs,26679computed hashes;342startup jobs and16jobs with
  a fresh pool-job tail. Only42hashes follow those342startup jobs in this
  retained prefix. No full-header correlation or mining acceptance yet.
  Zero native-filter matches, zero firmware pool submissions. API response
  buffers remain empty at later samples; no native dashboard pass.
- Compact ASIC trace implemented: C records actual received bytes at
  apply/reject/fault/reset boundary; all state/reply bytes and computed
  hashes remain. No RX duplication or TX edges. asic-packets CLI requires
  explicit backend contract; raw asic-events selects only raw event name.
  Auditor checks compact CRC/boundaries and rejects mixed streams. Its
  fresh independent SHA regression rejects corrupt/missing packets,
  changed boundaries, forged filter success and mixed formats.
  compact-audit-tests001 failed compile on Value/String ref comparison;
  fixed, tests002 PASS. compact-clippy001 found nested if; fixed,
  clippy002 PASS, fmt011 PASS, guide-links006 PASS.
- Freeze062 retains291sourcefiles; library
  2ed6f927c8ad8fb3b1312c8e04f02b758df61c115f649eb1f7ac05ef71890083,
  31817394B. Build062 UBSan RUNNING session97547. Next compiled burst,
  raw event equivalence, compact encoder equivalence, then original022
  with same fixed-mode console012 and asic-packets. Do not grant a pass
  before those tests. No model hashing/filter/timing change in this step.
- Isolated original bosminer --help012 failed before argparse because
  /dev/urandom absent in diagnostic init=sh. Program hash
  4997a1c10a09f6881d6f5e0d4967eb31072ea9f1da7a80dac192ae1299d45960.
  help013 confirms archived kernel has no devtmpfs. help014 RUNNING33654
  with tmpfs plus actual kernel null/random/urandom device nodes. This is
  generic direct-kernel diagnostic, not original-board acceptance.
- Latest user asked if complete: answered No in commentary and continued.
  Agents remain usage-blocked untilOct25 08:48AM; no unchanged retry.
  Full matrix still0exactpasses; no release gate is silently reduced.

- Build062 PASS: binary
  0e314cb4dafdb67d4132dee250e4206313556bb6311ed1f0638a048683c434de,
  19841880B; result exit0/no truncation. Compact reports copied to
  work/simulator/qemu-s9/backend-build-062-ubsan. Patch708b57e7409d47b0...
  c38, Cbindingca9d20d2c97397ac...e4a7 in frozen062 sources.
  Compiled compact-bm1387-qtest001 PASS1058responses. All23192raw events
  match061 exactly. All2518compact events match the actual raw stream.
  Reports compact-bm1387-{trace,raw}-comparison001.json. No firmwarepass.
- Cleanup021 reclaimed60verifiedduplicate buildCASobjects1143395114B
  from finished061/062 inputs. RestoreCOPYfrom031 perjournal. Keep061
  decbdcba library,0622ed6f927 library, backend binaries and unique recipes.
- Help014 PASS in generic isolated direct-kernel diagnostic. Actual
  original bosminer hash4997a1c...d45960 printed, actual --help executed.
  Original kernel lacks devtmpfs, so console-help003 creates kernel
  null/random/urandom nodes in private tmpfs. No native CLI timing option.
- Added s9_pool_work_audit.rs: independent full-header correlation. It
  validates actual Stratum notify/subscribe/version-mask bytes against
  retained pool contracts, then searches bounded extra_nonce2 counters
  and16versionbits to match entire SHA midstate AND12-byte tail. Hashes
  are recomputed with bitcoin_hashes, independently of modelsha2.
  This never submits shares or grants acceptance. Unmatched work explicit.
  tests003/004 PASS2; strictClippy003/004PASS. Initial021audit-v1 matched
 42headers; v2 additionally validates wire contracts, same42headers from
 10workjobs. Prefix remains truncated. README links audit for022 below.
- Original022 FAILED240051ms, no trace truncation. Fixed650MHz mode with
  compact traces;71893computed hashes,5281applied jobs. Independentfull
  headeraudit verifies34352hashes from4892workjobs matching fresh pool
  notify. Zero nativefiltermatches and0submissions. PIC55watchdog expires
  at108700034494ns while TXwork continues. API response buffers empty.
- Original023 FAILED240043ms, no truncation. Console013 derives from012
  and uses supported hash_chain_global.frequency=100.0. All3original
  guest logs confirm100MHz. It reaches sensor setup, reads chip61misc,
  then model fails PinRoutingUnsupported at~100216ms. This exposes the
  next device dependency, not an incompatibility.32954hashes/1085jobs,
  0poolsubmissions. Keep100MHz for next diagnostic; no clocks altered.
  There is an unresolved clock observation: original009 guest writes
  WORK_TIME0x543e5 at550MHz; model200MHz gives1.725ms vs legacy formula
 4midstates*2^19*.9/550MHz=3.43ms. This suggests separate timer/UART
  clocks but is NOT proof. Do not change timer based only on this inference.
- New explicit low-rate filter Policy component supports fixedbits1..32,
  native32 default. All guest ticket bits remain in original positions,
  readback unchanged, policy survives reset. Native firmwarebytes unchanged.
  CLI adapt-fixed-filter-bits=1..31 requires new backend contract/smoke.
  QEMU global md-sim-s9io.fixed-filter-zero-bits passes to Rust ABI.
  Audit reads declared policy and checks exact executed property. New
  pool envelope refers to run policy rather than falsely claiming native.
  Fullheaderaudit now calculates filter/pool intersection interval lower
  bounds at aggregate cap. This remains adapted-device execution.
- Filter tests001 failed nonexistent bm1387_work target (correct search).
  tests002 PASS; tests003 caught wrong test byte order in registerreadback
  (expectedLE but archived register isBE), fixed toBE. tests004/005PASS:
 31lib,5chain,6mining,3search,4wire,2audit (checklogs for exact count).
  Fresh serial adaptation test uses1bit testpolicy and2randomjobs; every
  hash is independently checked, real serialbytes compared, reset tested.
  qemu-coretests002/003 PASS5 includingnative32 andadapted1 fresh work.
  StrictClippy001PASS. New003trace event records model stop and retains
  actual received byte/hash events before an operation error. Priorcode
  discarded the failing call's Vec on Err; exactlastbyte/time test passes.
- Current freeze063/toolbuild RUNNING42305. Models/ABI/Cchanged, no063
  backend yet. Do NOT run latest C with062library. Afterfreeze, copy063
  lib to work/simulator/qemu-s9/libmd_sim_qemu_core.a, build063UBSan,
  run CABI ASan/UBSan and compiledburst/compact comparisons. Then run
  original024 withconsole013,compact,adapt-fixed-filter-bits=12 to test
  actualfirmware response path before the still-open sensor error.
  Sourceinspector whitelist now includes QEMU hw/sensor/tmp421.c andI2C
  core/header. Inspect pinnedQEMU for reuse, and retained braiins-monitor
  source005 JSON text sensor/tmp42x.rs,tmp451.rs,sensor.rs for sensorpath.
  No new sensorimplementation yet. Allagentsremainusageblocked.

- Freeze063 and UBSan backend PASS. Library bb10259345333d8726ff91873ecb8f5f54814a946901de3207bf266c809ed044 (31825130 B); backend 0fc9df9061c5c5aba799f9d15bc07d29ec5abe7c3dca202c99252b8332826788 (19837368 B). C ABI ASan/UBSan passes. Compiled adapted-filter-bm1387-qtest001 passes 1058 responses. All 23192 native raw events and 2518 compact events match062; comparison reports saved. Source count293. Do not rebuild this frozen identity from current sources.
- Added frozen private pool input schema2 with two distinct difficulty exponents (-31..0); default remains -20,-19. Tests4, strictClippy and fmt pass. Native ticket252 adds six bits to fixed32, so expected interval at full10000H/s is27487790.6944s. Original024 used explicit fixed4 adaptation and pool-28,-27, console013100MHz. Expected intersection interval0.10240156s at the entire cap; this is a lower bound, not measured throughput.
- Original024 FAILED (240s timeout, no trace truncation). Exact last failed packet now retained: link6,117287307770ns,4809f41c402041e017, PinRoutingUnsupported. Independent audits pass39223 computed attempts,1041 work jobs,34 ASIC filter matches. Full-header audit verifies14645 attempts from698 actual pool-work packets. Pool got4 frames and stopped parsing frame4 with 'expected value at line1 column2'; no submission accepted. Existing pool trace lost rejected frame bytes. Fixed retention: malformed frame now recorded as raw hex under separate invalid-json direction before failure. No malformed data is accepted. New regression/build RUNNING37466; next rerun025 withsame063backend/fixed4/pool-28,-27 to see actual bytes.
- Added standalone Rust TMP421 conversion/register model and8passing tests (tmp421-tests001). Not connected to ASIC/QEMU yet. New files src/tmp421.rs,tests/tmp421.rs. Timed sequential ADC, explicit timing profile, range conversion delay, coherent high/low reads, shutdown/one-shot, reset/power, channel enable, rate, ideal-diode n-factor correction, open/short/supply-invalid states. Need independent review, docs, bus integration, thermal input binding. No sensor or exact-target gate passed.
- Pinned QEMU TMP421 source inspected at work/simulator/qemu-temperature-source001; hash98901934bddf85f4c44f4cc771f31bcf2e98e03e944eb0bcddac3e99e1834799. Existing C model lacks ADC timing/one-shot/channel enables and changes range encoding only when external property updates. Reuse unchanged cannot provide required state/timing; Rust component permits direct integration with existing Rust ASIC and avoids second guest-facing C/FFI device path. Still need write decision in TMP421.md/README.
- Official TI TMP421 PDF SBOS398C (May2012), https://www.ti.com/lit/ds/symlink/tmp421.pdf, consulted through web; full PDF not saved/hash verified (network download remains blocked). Do not claim archived source. Archived Braiins sensor driver and ASIC I2C sources remain hash-pinned in braiins-monitor-source005/references.json and braiins-fpga-source005/members. Bus transport next: chip61,address244,misc402041e0,bottom bus, I2Creg20=flags/address/register/data BE, busybit31,commandbit24. Controller clock/reset/NACK behavior lacks exact silicon evidence; any reference policy must be explicit.

- Original025 FAILED240s/no truncation; actual model matches31 computed results. Audit40147hashes/1144work packets, independent full-header audit17238attempts from801fresh-work packets. New rejected-frame capture found raw prefix0900535452320141455347 immediately followed by a real mining.submit JSON (nonce00000008). Original guest logs identify its dev-fee StratumV2 probe. Pinned QEMU net/slirp.c hash9f6b0e5d1ef59bf1e3b9ea43b1cba50f71748dba5218e780c3ec2af93bce864c and qemu-options.hx c2306e120125c9c6648b9b690d0848c4f37c64ccb8f42d4861427b82dc8746b9 confirm chardev guestfwd shares one stream across connections. Do NOT strip the binary prefix; transport must retain connection boundaries.
- Added per-connection command pool: QEMU guestfwd cmd:/boot-runner __pool-session. Each connection has separate verified stream, trace and report. Bounded16sessions, own deadline/stop, collector emits all completed reports and incomplete IDs. Primary is first authorized V1 session by allocation order, independent of pass/fail. Pool7tests, runner6tests, strictClippyPASS. Partial frames now retained on close/stop. The hostrunner collects all sessions; independent audit still uses explicitly selected primary. This transport code is implemented but actual host execution is BLOCKED below.
- Sensor064 built: library c3972f62e24e6b0f1859ce25da36c05a1ef3a8df8009bd495362c5a0154f8a90,31953308B; QEMU dcd5ec5441cb3fe7e2b5bb958689fe755aff3003feeee1bbd2debb0a3e944150,19879000B. 299sourcefiles. UBSan build/smoke, CABI ASan+UBSan, component 1131responses passed, BUT THIS SENSOR MODEL HAD WRONG READ/WRITE DIRECTION. Native026 exposed it. Do not count064 sensor regression as validation of native protocol. Original064library/binary/test evidence retained. Details in TMP421.md.
- Original026 first launch failed before output creation: 'temperature-fixture' missing in CLI allowlist. Fixed; second launch retained026 FAILED240s/no truncation. Two separate defects: (1) libslirp per-connection exec uses an IPv4 loopback socket pair and gets Operation not permitted; no pool session starts. (2) actual guest sends controller command0198fe00 to read manufacturer. Model assumed conventional I2C direction and raised Sensor(ReadOnly254) at109138623845ns link8.
- Isolated extraction braiins-i2c-source006 supplies exact missing shared address helper: open/bosminer/bosminer-am1-s9/src/i2c.rs SHA2565cb55313d402cca4d61d1c03d6097a9e807298ea6e8a50335aa33b3cf8135ff1. It explicitly defines EVEN=READ,ODD=WRITE for ASIC controller addresses. Old summary inference that the comment was mistaken was WRONG. Corrected model and literal independent test commands; profile schema2 now hashes helper identity and direction. README/BM1387/TMP421 docs updated. Also fixed pin-routing changes during active transfer to stop explicitly, not finish through invalid routing. Original064fixture now rejected by latestrunner dueprofilehash difference.
- Current sensor065 tests/strictClippy/freeze/toolbuild RUNNING55611. On success copy065library and build065backend, run C ABI sanitizers and compiled bm1387-temperature; then native sensor path can run without local pool while transport is fixed. No065binary yet.
- Command transport host blocker confirmed primary libslirp4.9.5 source online, official mirror https://gitlab.com/qemu-project/libslirp/-/raw/v4.9.5/src/misc.c, release commit390848f47391f412451b2d259d8527fabb51e5e3/tree0509386ab50c032b58f5af4576f8ee5f79157c51. slirp_socketpair_with_oob uses AF_INET bind/listen/connect; denied in host. No fullsource archive downloaded. g_shell_parse_argv +g_spawn_async_with_fds means NO /bin/sh needed; removed unnecessary shell capture. stderr is passed to sameprotocolsocket, so child errors must not be mistaken forvaliddata. No unchanged retries. Prefer existing disconnected Linux VM workflow, where realTCP is already verified, rather than binary/interposition hacks.
- Transport next options: (A) run entire captured ARM runner inside existing isolated_network_vm, use existing namespace runtime withinVM. NestedTCG slower; need retain bounded compressed evidence and proper dedicatedUID1000/private run. (B) better: separate pool x86VM connected to ARM QEMU by inherited UnixStream pair via netdev stream. Hoststream initialization already verified by qemu_socket_link_probe; actualpacketexchange stillneeds test. Poolguest uses real LinuxTCP, preservesallconnections. Need captured Ethernet driver module (e1000 may be simplest), explicitIPv4 10.0.2.100/24/no default route; ARMalreadyconfigured10.0.2.15. Reuse isolated_network_vm initramfs builder and add explicitnetwork contract, do not bypass its lo-onlyguard silently. Kernel pinned /usr/lib/modules/7.2.9-arch1-1/vmlinuz SHA d02afb36d5f99390fdd8d04dedf8894354f4bbdc5e7d44afa5b897639bb3707c. Existing builder supports gzipbase<64MiB, decoded<256MiB andcapturedpayload. Its scenario files<=128,programs<=64; nested detection belowrun() needsinspect. No implementation ofnewVMtransportyet.
- Console014 derives013, keeps BusyBox nc stdin open2seconds using bounded guestpipeline, so it can receive native API reply before owned nc is killed after1sec. Not yetexecuted. Exact original scripts kept; newscript separatelyhashedbyrunner.
- Cleanup022 reclaimed60verifiedduplicateCASobjects1143395114B fromcompleted063/064inputs to031 canonical. Preserve031CAS;restoreCOPY. Keep063bb102library/064c397library andallbackend binaries/uniqueinputs. /tmp back~36GiB,root~1.9GiBfree.
- Agents remain account-usageblocked untilOct25 08:48AM. No independent review pass. Full frozenmatrix79,599rows remains0exactpasses. User latest asked completed: answeredNo andcontinued; nofinalresponse.
- Sensor065 tests, strict Clippy, source freeze and backend build PASS. Frozen library209ff967ed931f0063df6af77410d735e12d9272ae1c34dcfbac9ae4ad09424b (31968756 B), backend88b14510d272d04a6c1b6b42ecb7f042ea737655d920b56b5f528138eb2e4c54 (19874008 B). C ABI ASan/UBSan PASS. tmp421-bm1387-qtest002 passes1131responses with controller EVEN=READ/ODD=WRITE. Compact build reports copied under backend-build065. Native027 not yet run.
- Implemented pool TCP accepted-stream entry point and shared frozen pool input contract. New s9_pool_guest runs only in captured Linux VM; loads hash-pinned e1000 kernel module, enables one explicit private IPv4 interface, rejects default route, drops to UID1000 with resource limits. Real independent pool verifier handles each TCP connection separately. Optional test client sends a separate binary probe, reconnects twice, checks actual invalid-submission rejection; no mining answers. New support/pool_vm.rs captures ELF closure/kernel/initramfs and BIOS; s9_pool_link builds image and runs two private Ethernet VMs via inherited Unix socket pair. isolated_network_vm --prepare-only saves hashed prepared image. StrictClippy002/build001 PASS after correcting imports and Store API use.
- Private-vm-link001 FAILED: both Linux VMs booted, driver loaded, server listened, but client connect timed out and zero TCP sessions arrived. Do not infer packet exchange from QEMU starting. Guest panic makes QEMU exit0, but assessment correctly rejected missing evidence. Added packet captures and guest route/ARP/carrier/error diagnostics; source inspector now includes net/stream.c,io/channel-socket.c,util/qemu-sockets.c. Build002/source inspection running95317; next link002 will use diagnostics. No board/remote/mining gate passed. Agents still blocked by unchanged account usage limit.
- Private-vm-link002 FAILED with full packet captures: client eth0 UP/carrier1, correct10.0.2.15/24 route,12TX/0RX, unresolved ARP. server/client pcap saved. Captured QEMU stream code shows asynchronous initialization can return without a working endpoint; it queries socket addresses. Host queries are restricted. Do not repeat an unchanged native stream attempt. qemu-fd-source002 retains net/socket.c baseline182194a1b5c0ffb8f94ff5dab41647ac0775215fcb6d58c0aa6d7fd1bbc5d87d, qapi/net.json01f716c110d0b5f4cc21468cf76b32c4da2bf3658b0bc405e5c65dfea8727f08. qemu-pool-source001 adds exact x86 Kconfig; I440FX selects PC/ACPI, E1000_PCI, SERIAL_ISA explicit minimal support profile.
- Added generic process_vm wrapper: captures exact ProcessSpec and all input objects in a disconnected Linux VM, drops guest runner to UID1000/no capabilities, invokes existing namespace runtime unchanged, exports bounded compressed result in ordered/hash-checked chunks. Outer has no NIC. isolated_network_vm gained explicit captured-process profile (3GiB virtual RAM,2vCPU,5GiB address limit,1440CPU/900wall,64MiB trace); inner process limits stay unchanged. build/clippy001PASS. This is local virtual topology, not external physical host. s9_boot_bundle prepare-only now freezes full existing process contract without executing.
- Process VM link001 (/tmp/md-sim-process-vm-link-001) transported full matching result. INNER TEST FAILED: both nested Linux guests exceeded55second startup budget before init. Outer completed~66s/120CPU seconds. No Ethernet success claimed. The wrapper did retain actual complete failure evidence.
- Original027 prepared with065backend, sensorfixture, fixed4, pool-28/-27,console014. Inner specSHA f62efa95032c285fb28c4fcee33271e1f49c34ff273d6ed8e70cd2f141a2c10d. Ran via process_vm at /tmp/md-sim-process-vm-original-027. FAILED before console activation: actual original SPL/UBoot/kernel/earlyuserspace executed, loaded squashfs, reached blankNAND UBI scan; CPU exhausted beforeminer startup. Child usage~221192622us user +12974236us system, SIGKILL. No trace truncation. Pool230s expired with0sessions. Outer250583ms,281167230us user/3202514us system,maxRSS1621204KiB. Do not report board/mining success. No further unchanged nested test planned.
- New explicit QEMU private-stream.patch reuses net/socket.c framing/parser/queues andpartial writes. Option socket,fd=3,x-md-sim-private-stream=on requires inherited socket fd; launcher supplies connected AF_UNIX/SOCK_STREAM pair. Uses POSIX read/writev only; no address query/listener/address fabrication. Ordinary socket modes unchanged. No CPU changes. Documented PRIVATE-STREAM.md; not yet compiled/packet-tested. build_qemu_backend now has optional x86-pool mode using minimal I440FX/E1000/ISAserial on same pinned QEMU. Next build both ARM066 and x86pool with UBSan, then actual twoVM Ethernet test. Active clippy/toolbuild/freeze066 session49373. Do not edit frozen roots until checkpoint finished.
- Stratum regression7tests PASS; boot runner6tests and isolated VM evidence test PASS. Independent review remains blocked by agent usage limit. Fullmatrix remains0exactpasses.
- Cleanup023 reclaimed30duplicateCASobjects571697557B fromcompleted065 inputs to031 canonical. Keep031CAS and065unique library/binary. Current /tmp~37GiB, root~1.1GiB free. Root work data is not in /tmp; preparation copies count toward root disk.
- Private stream adapter now builds and passes packet tests. Final patch SHA256 e075755129de7c70a19a19aaf1807eb952981c233f28c65bee289aebebe1d0aa. Pool builds001/002 failed QAPI documentation checks (line width, then sentence spacing); both failures remain. Pool003 PASS: x86 binary a6b79923e6e111041a7197bf22f9080c4ec125bd64edf8398b47d98ed4505ad2,19064832B. ARM066 PASS: 3d5be0a3deffc545487e9060a4a4c5a8ed57f3a5984f2dee6628bfc50a5c2461,19884344B. Frozen066 model library89fca75b3b2c7131b6cd0b1e273a0570790bce5ccd2c49bdc2bdd1bd4e6d7302,32027816B;306sourcefiles. Freeze preceded the QAPI text fix; build report separately hashes the exact later patch. C ABI ASan/UBSan PASS; compiled tmp421-bm1387-qtest003 PASS1131responses. This is not a frozen release identity for later host source changes.
- private-vm-link003 PASS; stricter repeat004 PASS at /tmp/md-sim-private-vm-link-004. Two separate full-system Linux VMs exchange actual Ethernet/TCP through inherited Unix socket pair. Binary probe stays in its own session; two V1 sessions reconnect and receive invalid-parameter rejections. Full pcaps saved. Tests require both guest completion/poweroff markers, empty QEMU stderr, exact three sessions and zero mining submissions. This is local VM transport evidence, not external physical-host or mining acceptance.
- Original028 at qemu-s9/sd2022-private-pool-028 EXITED0,237263ms,no trace truncation. Unchanged archived SPL suffix, UBoot, kernel and native bosminer executed with explicit sensor/fan/presence,100MHz and fixed-filter4 reference adaptations. console-private-pool014 completed; original API now returns real JSON. Independent pool selected primary session0 by authorization/allocation order and accepted two different fresh jobs, with clean transition and difficulty change. One real ASIC result was rejected LowDifficulty. All nine connection reports retained; six V1 sessions completed, three binary-probe sessions closed. This is a milestone, NOT an exact-target or original physical-board gate. BootROM, blank NAND fixture, secondary CPU adaptation and FPGA/physical-profile identity remain unresolved.
- Original028 hash audit independently checks233825computed attempts across12640work packets (chain6:82670,7:75667,8:75488). Initial pool-work audit FAILED 'pool request failed' on the legitimate LowDifficulty rejection. Fixed that distinction with a regression. Added separate submission audit to replay exact wire requests/responses and correlate full headers with computed nonces and actual CRC-checked serial output. New audit not yet run on028. Do not claim full hardware-to-submission correlation before that passes. Regtest support being added via explicit genesis anchor in pool-input schema3; ordinary028 jobs have random parents and are NOT Core candidates.
- New checks: process-vm-recovery-tests001 PASS2 (failure evidence plus tampered/truncated/conflicting transport); private-stream-clippy001/002/003 PASS; private-stream-link-build004 PASS; guide links371 PASS. Later audit/schema edits need affected tests and Clippy again. Agents still fail unchanged account usage limit untilOct25 08:48AM, confirmed list_agents; no independent review pass.
- Cleanup027 reclaims40verifiedduplicate build objects1203780547B from completed065/066/pool001/002/003 to031 canonical CAS/overlay/keycodes. Preserve031 and all unique backend/model binaries. cargo clean -p md-sim-firmware --profile dev reclaimed2GiB derived caches; logfirmware-dev-cache-cleanup001. Currentroot~2.5GiBfree,/tmp~37GiBused. Emergency idle shell70212 remains. Do not remove canonical inputs or archive files.
- Current action: build new submission audit, s9_boot_bundle and s9_pool_guest (session64779). Test and run audit028, then run new original firmware with explicit regtest-genesis input and correlate its accepted headers before Core import. Need adapt node_prerequisite_probe evidence linker for original reference execution; never label it source-adapted cgminer. Fullmatrix still79599rows,zero exactpasses.
- Submission audit028 PASS. First audit001 checks233825hashes,211complete ASIC serial responses and all3actual primary submissions (2Accepted,1LowDifficulty). Each full header matches active guest work, independent SHA256d, actual filter, nonce ownership and CRC-checked queued serial bytes. Identical work can occur on several chains or reconnects; all real matches retained (6,4,6). No cross-clock ordering invented. Updated002 also binds guest/backend logs to exact result bytes, pool sessions to actual runner output, and pool input to the executed mount. pool-work-audit002 PASS46363fullheaders/2722matchedworkjobs; other work remains explicit. Regressionfixtures derive from028 with parenthashes, not new mining evidence.
- Pool schema3 adds explicit RegtestAnchor::genesis; schema2 remains readable only without anchor. Tests reject changed anchor/time/oldschema misuse. Both command and VM transports use the explicit input. s9_boot_bundle regtest-genesis requireslocal-pool and recordsanchor inadaptation/input. New s9_asic_audit regtest performs actual submission replay before exporting blocks. node_prerequisite_probe supports this tagged original-firmware proof separately from source-fixture arrays, reruns companionaudit, copies verified source logs/contracts, validates exactblocks and runs same3-phase originalCore test. No firmware/miner replacement and no node nonce search.
- Original029 FAILED240039ms wall timeout,not truncated. Poolprimary DID accepttwo freshregtestcandidates and rejectone stale submission, but consoleonlyreachedsample12. CompletepoolVM reportsretained4sessions. No successfulexecutiongate claimed; newaudit deliberatelyrejectsfailedexecution. Runner nowpermits360wallseconds whileCPU remains<=230seconds andpooldeadline<=230seconds. Ordinaryguest/bus/watchdog/thermal clocksunchanged. Neededwallmargin followsmeasuredfailure,notrelaxed assertions.
- Original030 PASSexit0,237082ms,no truncation. Actualcommand: target/release/examples/s9_boot_bundle boot work/simulator/original-zynq/sd-bundle-006 work/simulator/qemu-s9/sd2022-private-pool-030 spl sd2022-v1 backend=/tmp/md-sim-qemu-backend-066-ubsan budget=360 presence-pin-fixture=e0 fan-fixture pic-contract asic-contract asic-packets local-pool pool-vm=/tmp/md-sim-qemu-pool-003-ubsan temperature-fixture adapt-fixed-filter-bits=4 pool-difficulty=-28,-27 regtest-genesis console=work/simulator/qemu-s9/console-private-pool-014.sh. SpecSHAea02b7f3ef22e94e87240499bb9c91277582e590da2f2b20f54efcb812517dd4;poolinput26ed1f9050cff440ce21f474531a386b225d2bc5978ab95f45cbe1d2fc65f1a4;adaptation5b9c1cda0a912834d3483d44fec0209f9af54e28bc91b6b73d8c188c5a9411a7.
- Original030 regtest-audit001 PASS278156actualcomputedhashes,275completeCRC-checkedASICresults,twofreshacceptedsubmissions,cleantransition,difficultychange. Bothsubmittednoncesare0andwereactuallycomputed; eachhas6retainedhardwarematches. Candidate0hash086e3eac036590d571352ab00c419f455c1d9caf2fcc6d5dc5d214816041c8c2;candidate1hash02bfdbfb01f318453eff7d3f6e052f42abe2ad827c69db30af8d90e024fc74b2. Independent Coreaccepted BOTHin separatefreshchainstates andloadedexacttipafterrestart. Results bitcoin-core/original-s9-030-candidate-{0,1}/block-acceptance.json. Commandsnode_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind NEW_OUTPUT work/simulator/qemu-s9/sd2022-private-pool-030/regtest-audit-001.json INDEX, actuallyexecutedfor0and1. NoCore-generatedblock. Bothremainadaptedreferenceenvironmentclaims,NOTexactphysicaltarget/fullboard/conformance/calibration.
- FreshCorelink repeats hardwareaudit beforeimport; companionbinarySHA497e5cd904a69fbf3f11349b689db3350b593b4314393fa614630b076b50be4a,1706664B. Independentreviewagent remainsblocked; do notconvertself-review toindependentreview.
- Checks original-regtest-fmt001PASS; original-regtest-clippy001PASSworkspacealltargets; original-regtest-tests001PASS85testbinaries/284tests/0failed/0ignored. PythonBitcoinfixture001PASS. Earlier affectedtests002/003,strictClippy001/002andreleasebuild003PASS. Alllogsunderchecks-2026-10-21-001. README/AGENTS/progressupdatedwithoriginal030milestone,needrunlinkcheckafterlastdoceditandcheckAGENTSlinksmanually. LatestRustchangesalreadycoveredbyfullworkspacechecks. CABI/compiled066sanitizerchecksunchangedandpassedearlier.
- Copied066ARMand003pool compactbuildreportsintosimulator/qemu-s9/backend-build-066-ubsan andpool-build-003-ubsan. Addedprivate-vm-link004symlinkto/tmp/md-sim-private-vm-link-004. Existing003evidenceunchanged. Pool003buildreportoldlimitationsstringwronglysaysZynq; newerbuildhelpercorrectstargetdescription,butdoNOTrewriteoldtestedreport. Actualtarget/binary/machinesmokeidentifiesx86.
- Extendedreclaim_qemu_inputs tofinishedprivate-poolrunstoreswithstrictpathallowlist,resultpresenceandStoreexclusive locks. Firstattemptcorrectlyrejectedpersisted512MiBstorepolicyagainstold2GiBbuildpolicy. Fixedselection; secondattemptrejected027prepare-onlystorewhichhasnoouterresult.json. Noobjectsremovedbeforejournal. Finalcleanup028PASS186verifiedduplicates799444949B from024/025/026/028/029toCANONICAL030input-store. KEEP030completeinput-store! journalwork/simulator/qemu-run-cleanup-028.json hasrestorationCOPYpaths,neverhardlink. 027preserved. Testscheckexplicitallowlist/archiverejection. Root~2.8GiBfreeatlastcheck; /tmp~37GiB. Existing031buildCAS,004SDcanonicalandallothercleanupcanonicalsstillprotected.
- Remainingpathdefects: nativefirmwaremonitorlabelsfunctionalrun'slowhashrateas'sick'; testpoolclosesaftertwojobsso'remote disconnected'/'no active pools'istrueexpectedtestbehavior,notfakehealth. Do notdisablewatchdogsorclaimhealthyproductiontelemetry. Noadditionaladaptationyetbecausewarningsdonotpreventrealmining. Neednextimplementrealboardpersistentstorage/export/restart andnativeboarddashboardpath,plusmissingfault/update/conformance/identitywork. Existingnative024browser/SSH/TLS evidenceisadifferentexecutionidentityandcannotfill030gates. Fullmatrix79599rowsstill0exactpasses; keepiterating,no final.
- New SD persistence work is IN PROGRESS, not passed. Added support/s9_boot_storage.rs: bounded gzip snapshots of the original boot's qcow2 SD overlay, exact original backing hash/128MiBgeometry, inputrestore via sd-state=SNAPSHOT_JSON, digest/size/path checks, saved SD states across emulator processes. s9_boot_bundle now supports save-sd and up to2restart-console scripts (currently no local-pool with restart scripts). Native guest sync precedes emulator process termination; NAND/volatiledevicesreset. Snapshotmetadata retains that limitedscope; nohost-crash claim. Snapshotscontainrawqcow2 hashes, compressedhashes andisolatedqemu-imginfo. Actualbytesretained eveniflaterguestscriptfails. CLI/adaptationdocs stillneedcleanup beforefeatureclaim; currentconsole.jsonrecordsstate/restarts/snapshot scope,adaptation.sd_storageoldertextstillonlytemporaryneedsfix.
- Added Rust s9_boot_storage_cycle driver: fresh256-bitmarker/nativeUCImax_connectionsvalue;threeoriginalSPL/UBoot/kernelstartswrite,read+restore,thenverifyrestoration. Ownscriptfixturesuseguestash/UCI(existingREADMEexplainslanguageexception). DriverchecksactualCRLFmarkers,threeexactbootbanners,andoriginal/restored/finalguestfilechecksums. Newunit tests8bootbundle+1driverPASS. FirststrictClippy001FAILEDnonminimalboolean;fixed. Tests002/Clippy002/releasebuild002PASS. NewchangesNOTyetcoveredbyanotherfullworkspacecheck.
- Actual original-sd-cycle001 FAILEDcorrectly: firstnativeguestwritesvalue280andfreshmarkerff73a21694b5456efc5e1750aa5386ca9ae27d31a8f580a5643429c689d4939e, but secondbootcannotfind/etc/md-sim-sd-marker. No thirdboot. Actualwrapperexit1,assessmentfalse. Bothbootsmountmmcblk0p2thenmount_rootrootdisknotformatted,loop0ext4,no journal,then'switched to extroot'. Needtrace/probe/provewhere/etcwriteswent; doNOTweakenassertions. TwoactualSDsnapshotsretainedinoriginal-sd-cycle001/run: raw0e34e6f5e507a66fd2aedaacdf3598e7de2cd44892f5f4075608c64e977121af9,raw11cd477a31a099cc7a16448d2c707b7e546930d080f7d7b90029a517222403df3,both917504B;gzip0120608Bhash1d25e6ba1ddaeaf2b4bca34bca6d507e99ee469ff9432632b28dc1f4aa2a30cc,gzip1117353Bhashd83eaa011c3baf8ee4bc88cca025f7e818dca27ecd8a19d95c1d4afa6deb7d0d. qemu-imginfoqcow2dirtyfalsebacking/sd.img/rawand128MiBcorrect. Original030Coreevidenceunchanged.
- NEXT: inspectactual/proc/mounts,/proc/cmdline,/etc/config/fstab,/overlay paths beforewrite/afterfirstsync/secondboot. Use explicitdiagnosticwithsd-state=snapshot0toexerciseexternalrestore andretainfailureifmarkerabsent. Needindependentdiskoracle: capturedqemu-imgconvertqcow2torawINSIDEisolatedruntime,thenexisting7zinspect/extractpartition2/ext4andmarker/config. Neverexecuteacquiredimageonhost. Existingpreparation/s9_storage_reportpatternsavailable. SuspectfirmwareoverlaysequenceorSDmodelwritepersistence;notresolved. Runtime3bootsusesnewQEMUprocesssameqcow2file,notrecreatedbetweenstages. Innerloopusescommandargsunchanged. Preservefailedsnapshotandinputidentities.
- Latestguide380linksPASS(original-regtest-links002). BitcoinfixturePASS. AGENTSlinksREADME/docs/spec/designexistcheckedread. SourceREADME/AGENTSnowmention030originalreferenceCoremilestone,notexacttarget. Root~1.1GiBfreeafternewrun; /tmp~37GiB. Fullworkspace284testsPASSwasbeforeSDfeature; rerunaffectedthenfullafterfix. NodeCore0/1bothpassed. Agentsstillusageblocked;continuewithoutfinal.
- Storage root cause traced to pinned QEMU source: SD sd_blk_write calls blk_pwrite(...,flags0), no guest flush. BlockBackend defaultswritecachetrue; disablingwritecache addsBDRV_REQ_FUA. Capturedqemu-sd-source001/references.json: hw/sd/sd.c SHA057427f10ea5f6ff9fb48f769230cf19b21e9d5f97ad3bf78afbc7f8fa78f657;block/block-backend.c8bae44d4cad82762558afc79fb5a478d6460b158caac3ff985161f3638e245f5;block/qcow2-cache.c2307ee169a1b00780bb0af86f92487977a1de4ddd20b898c62670dced5f5020e. DefaultwritebackplusforcedQEMUterminationlostcachedimageupdates despiteguestsync. Changeddriveexplicitcache=writethrough; recordedconsole/adaptationpolicy. NoCPUemulator/devicepatchorarchivedfirmwarechange.
- original-sd-cycle002 PASS84852ms,3actualoriginalSPL/UBoot/kernelboots. Freshvalue953/token5864eb4509ef4e24a0efdb70919cd83bc598cae9adb5b961d65473ae7e1efcdd persisted;secondbootread953andrestoredfile;thirdread100andoriginalSHA302128daf55df98b268cc7296326a8fd33786747da9b3ca58762730aa675b077. Actualcommandtarget/release/examples/s9_boot_storage_cycle work/simulator/original-zynq/sd-bundle-006 /tmp/md-sim-qemu-backend-066-ubsan work/simulator/qemu-s9/original-sd-cycle-002. 001retainedfailure,neverrewritten.
- External snapshotrestore003/004 PASSexit0. Bothnewisolatedrunsuse s9_boot_bundle boot ...spl sd2022-v1 backend066 budget180 save-sd. 003sd-state=original-sd-cycle002/run/sd-snapshot0.json withconsole002/console1.sh reads953/restores100;004loadsstate2withconsole2.sh verifiesrestored100. Exactfilenameshavehyphens asactualdirectorylisting/commandsinlogs. Allsnapshots/proofkept. This testsSDonly; NAND/volatilemodelsreset. Nohost-crash/whole-boarddurabilityclaim.
- AddedRusts9_sd_snapshot_inspect SNAPSHOT_JSON NEW_OUTPUT. CapturesoriginalSD,qemu-img,7z,libsandinputstate;insideclosedpreparationnamespace restoresqcow2, readsMBR viaqemu-imgdd,extracts64MiBpartition2,thenindependent7zext4parserreadsmarker/config/backup. Noacquiredcodeorimageutilityexecutesonhost. Exactgeometryrequires2022SDstartsector83968,length131072. Sourceimg_dd countboundsINPUTendbeforeskip,soactualcount=start+sectors; sizeschecked. Inspection001ofcycle002state0findsmarker65Bhash5ec3179b743ec3e324f3b8ee33093dfc4ce2899444aab4e0211cf33ba057234d,changedconfig580Bhash7030e17f58670ab2db79d7ce8de1f598774da9d4a664729b73fd41f3e950f7c3,backup3784BoriginalSHA302128... Inspection002ofstate2findsrestoredconfigoriginalhashandnobackup/marker. Inspection003offailedcycle001state0findsNONEofthosefiles,independentlyconfirminglostimagewritesbeforesecondboot.
- SDClippy003caughtunusedimportandunusedprofilefield;removedimportandrecordedboot_memberidentitiesininput-origin. Clippy004/build005PASS;inspectorbuild006PASS. EarlierbootSDunit8+driver1PASS. Needfullworkspaceafterlastchanges. Snapshotdrivernowbindsguest.logtoresult.stdout. Adaptation.sd_storagechangedfromoldtemporarystringtostructuredcache/snapshot/restart/inputscope. Thesechangesrequirefreshminingrun;existing030evidencekeepsoldtestedidentity.
- PreservedoldS9auditorbinary+fourRustsources+Cargo.lockunderwork/simulator/verification-builds/s9-audit001/SHA256SUMS(exactactualpath s9-audit-001). Binary497e5cd9... describesold030proof. Fixedsubmissionauditorresultreadbound64MiB→128MiB,becauseProcessResulthexencodes64MiBtraces(hashauditalready128MiB);actual030result66288869Bwasnear64MiBbound. Backend/guestbyte-logboundstays64MiB. Newauditbinary/sourceidentityinvalidatesreuse;reauditnewrunsbeforeCore.
- ACTIVE session13212: affectedSD/miningtests,fullworkspaceClippy,releasebuildthenoriginal031. Commandsame030plus save-sd, withnewwrite-throughpolicy,currentrunner/currentauditor. Outputwork/simulator/qemu-s9/sd2022-private-pool-031,logschecks-2026-10-21-001/{sd-and-mining-tests001,sd-and-mining-clippy001,sd-and-mining-build001,sd2022-private-pool031}. Afterfinishrun s9_asic_audit regtest031newreport,thenCorebothcandidates;resumeoriginalmatrix/UI/fault/updatesafterstoragecyclechecks. DoNOTmark030aslatestcodepass.
- Devcachecleanup002cargo clean -p md-sim-firmware --profiledevfreed~1.6GiBafterfinishedchecks;release/capturedinputsintact. Lastroot~1.8GiBfreebefore3inspections/new031; /tmp~37GiB. Preserve030input-storeascleanup028canonical. 031newstorelikely~200MiB. Independentagentsstillblocked. Noexactphysicaltargetpass. No final.

- Original run 031 failed after 252103 ms. Its console reached sample 25 of 30, then QEMU received SIGKILL at the 230-second CPU ceiling. Cumulative waited-child usage was 230683672 user microseconds and 2617836 system microseconds; this includes the pool VM. Two pool jobs were accepted, but the full execution gate failed. The failed result and SD snapshot remain unchanged. No mining acceptance audit is claimed for this run.
- Review found that schema-1 snapshot scope text claimed native sync even after a failed console. New schema 2 has a required `guest_sync_completed` observation. Failed images can be recovery inputs; integrity does not establish a clean shutdown. Successful schema-2 retention requires completed sync. Historical schema-1 records are preserved, with the failed-031 scope error documented in SD-STORAGE.md.
- The mining board guard now permits 360 CPU seconds; the runner caps its selected allowance at 330 CPU seconds. Wall bound remains 360 seconds and pool bound remains 230 seconds. Guest clocks, hash cap, ASIC filter, watchdogs and console assertions are unchanged. Tests and workspace strict Clippy pass in checks-2026-10-21-001/sd-mining-budget-{tests,clippy}-002.log. Release tools build passes in sd-mining-budget-build-002.log.
- ACTIVE original run 032, exec session 86197. Same exact archived SD and ARM066/pool003 backends, console014, explicit fixed-filter4, temperature/fan/presence fixtures, pool difficulties -28/-27, fresh regtest-genesis work, save-sd, budget360. Output qemu-s9/sd2022-private-pool-032. Next: require complete execution, run current s9_asic_audit regtest, then submit both actual candidates to independent Core nodes. Do not count an incomplete run or shares alone as a pass.
- All three independent agents remain errored by the account usage limit until 25 October 2026 08:48. No unchanged retry and no independent review pass. Full frozen matrix still has zero complete exact physical targets.
- Run 032 completed the console and native sync, saved schema-2 SD state, and exited zero after 237497 ms. It still FAILED: stderr reached its old half-budget limit of 33554432 bytes. The total retained output was much smaller than the 64MiB process allowance. The independent mining audit correctly remains unavailable for truncated execution.
- Changed the generic trace collector to share the same aggregate byte allowance across stdout and stderr. Atomic reservations prevent concurrent overrun. This does not raise the 64MiB worker policy or discard an event class. Added asymmetric-output, exact-bound and concurrent-overflow tests, plus real isolated execution with exact live-file comparisons. Existing overflow fixtures now exceed the full aggregate budget and assert its exact size. Test attempts001/002 exposed old half-budget expectations; corrected those expectations without reducing overflow checks. Runtime all-target tests003 and workspace strict Clippy003 pass. Release build003 passes. S9 result audit bounds now allow128MiB of hexadecimal data plus1MiB of metadata.
- ACTIVE original run 033, exec session12745. It repeats032 with the shared trace budget and will run the independent regtest audit only if execution completes without truncation. Same archived firmware, backend066, pool003, console014, fresh work and explicit adaptations. Next: inspect result, then Core acceptance for both candidates.
- Verified dashboard provenance: SD bundle FIT SHA8b2a32020281aa67490c01633adf99c1a2ca6c9897e4c210bb6d0d001c15caca equals the pinned update FIT from original_zynq_probe. Its ramdisk is2bd49c70b915b7e25f6c01013d95a5ffea8d499b73c6c6ec5539fda4247154b3. Original dashboard app SHA1c318483df06a03e460e35f7419d8562c007f5dc4965a20d412c96392ad36423 is425409B. It uses native /graphql and session_id cookies. Isolated extraction original-shadow012 confirms archived root has an empty password. This is reference evidence only; live original-board dashboard tests remain to implement.
- Original033 PASS: exited0 after237272ms, no trace truncation, native console014 completed and schema2 SD sync/export retained. Independent regtest-audit001 PASS267118actual hashes,249complete CRC-checked ASIC replies,two fresh firmware submissions,clean transition,difficulty change. Both real Core tests original-s9-033-candidate0/1 PASSheight0→1 and exact tip after restart. No exact physical target or full-board gate inferred. README and SD-STORAGE.md link these results.
- Added optional `dashboard-probe` to original boot with pool-vm and pinned2022 image only. Separate Linux client uses captured curl, a new cookie store, native HTTP page/asset hash, GraphQL rejection/login/uptime/logout checks. It preserves all request/response bytes and reports scope separately. Full browser/configuration/WebSocket/upload/SSH/TLS gates remain open. Client source is examples/support/s9_dashboard_probe.rs. Pool preparation captures the exact helper, curl closure, contract and other inputs. Added response-assessment negative tests. Dashboard-probe-tests001 and workspace Clippy001/002 plus release builds001/002 PASS. The actual dashboard path has NOT yet passed.
- Pool VM now exports its actual bounded packet capture after stop; outer runner verifies the digest and retains pool.pcap. Earlier mining runs retained only its digest, which is not the packet evidence itself. Existing separate link tests already retained full packets. Optional dashboard and packet retention changed runner/pool identities after033; do not call033 a test of these later additions.
- ACTIVE original034 exec session1703. Same033 full mining/storage options plus dashboard-probe, fresh pool input. Output qemu-s9/sd2022-private-pool-034. Next inspect actual native responses and fix any failures, then require complete execution, independent mining audit and both Core blocks for its own identity. Full workspace tests still needed after final fixes. Root~2.2GiB free before034; /tmp~37GiB. Keep all cleanup canonical stores.
- Native034 FAILED its optional dashboard check. Actual HTTP index200 (2044B,SHA d0d769811cbf2e5290dafb5f9f9a005bcafd45469e94899080353093e1f20947) and archived app200 (425409B,exact SHA1c31848...) passed. Native auth.extend without a session returned AuthError. A root login with an arbitrary password returned VoidResult: the archived root account has an empty password. The initial test assumed AuthError for that input and was wrong. The exact archived frontend also explicitly handles VoidResult as login success. Keep034failed; its observed behavior is retained in dashboard-result.json, not reclassified.
- Corrected dashboard contract to test an unknown user rejection, then the archived root empty-password login, session extension, live uptime, logout and invalidated session. Success uses native VoidResult. This does not claim that the default passwordless account rejects arbitrary passwords. Tests003, workspace Clippy003 and release build003 pass.
- Review found another completion bug: QEMU can exit0 when the VM init reports a failed guest command. Pool finish now requires the two actual guest scenario/completion markers and empty backend stderr, as the existing link suite already did. Shared helper removes duplicate checks. Regression rejects shutdown alone, missing/duplicate markers and backend errors. Earlier034 outer ProcessResult was0 but its host dashboard assertion correctly returned failure; new runs also propagate the failed VM scenario into ProcessResult.
- ACTIVE original035 exec session10748, full original mining/storage plus corrected dashboard-probe. New input identities. Output qemu-s9/sd2022-private-pool-035. Full workspace tests and fmt also active; logs original-dashboard-workspace-tests001/original-dashboard-fmt001 under checks-2026-10-21-001.
- Cleanup029 uses KEEP canonical033input-store and reclaims only matching immutable objects from finished031/032/034. Check journal completed flag and exact count after command. Restore with COPY, never hardlinks. Canonical030 fromcleanup028 remains intact too. Full pcaps first retained in034 (569856B); current failed GUI gate does not invalidate packet bytes.
- Full workspace check original-dashboard-workspace-tests001 PASS87test binaries/291tests/0failed/0ignored; fmt001PASS. Strict Clippy003 alreadyPASS. These Rust checks precede the later clock-source inspection allowlist only. Dev cache cleanup003 freed1.6GiB; release binaries and captured inputs preserved. Cleanup029 completed126duplicate objects/581963772B, canonical033 remains complete; canonical030 also protected.
- Original035 FAILED overall, ProcessResult exited1 after349752ms without truncation. Native dashboard component passed11actual HTTP requests: index and exact app asset, invalid session, unknown-user rejection, root empty-password login, successful session extension, uptime78→81 with unchanged boot time, schema query, logout and rejected extension. Keep this as component evidence only: overall boot/mining path failed. No complete dashboard or target gate.
- New actual035 failure: ARM066 stops with `qemu: Fan controller: unsupported model operation` at about98.143virtual seconds, before completing mining startup. Last fan MMIO writes were valid0x64 at0x10/0x14 with error0; no failing MMIO event appears. Thus investigate the fan timer/clock advance path; do NOT assume unsupported duty or clamp values. Script stalls at sample10; no successful mining audit/Core for035.
- Added error diagnostics to md-sim-fan.c for backwards clock, rejected elapsed advance, and failed write address/size/value. No model behavior is changed yet. Source freeze067 completed314files and retained the Rust library. ACTIVE exec44565 builds /tmp/md-sim-qemu-backend-067-ubsan from pinned QEMU and canonical031 overlay, using samurai-build003. Log backend-067-build.log. Frozen067 predates the later inspector allowlist additions; build tool captures its exact inputs separately. Next require UBSan build/smokes, fan qtest and C ABI ASan/UBSan, then reproduce035 with067 and inspect the precise failure instead of guessing.
- Isolated qemu-fan-clock-source001 inspection succeeded. Exact pinned source hashes: system/cpu-timers.c dc7d022db8e17a6323285e78897f9f8f15c537931f7bcd46094373f494758671; util/qemu-timer.c2b8436271d4638cf97927981c1ab26934488c3c819e44d6aa5b870fa0621317c; system/memory.c ec1106c0249bf8f9f7a33f9756962d689bebcdd9c95c36876be2336d1044f5cc; accel/tcg/cputlb.c f5aa5cc4d5752aa179cac7344b58164ebbdf1c0451154fe75dc73f2dcf1a3a37. These were read inside the preparation namespace. Clock and MMIO locking cause is not established. No timer period or guest clock adaptation added.
- Next native UI work after fan defect: native GraphQL write/readback/restoration/restart persistence, WebSockets, browser rendering and selected-run SSH/TLS. Existing native024 is a different execution identity and cannot satisfy these. The new client still only probes HTTP/auth/uptime. Optional dashboard-probe is implemented but still missing from the long CLI usage string; update it with tested documentation after a combined pass. No final; all completion gates remain required.
- Backend067 UBSan build and all declared smokes PASS. Binary c57fde7de1b31cfeea9bb6109872e4edc6fd879be1e7028f2ae7646dada01d0b,19879576B. Frozen model library6f391af713b832b2415d911a5635b50bb75c9f87c8c695256ef3b89229aaf2b7,32053342B. Compact build metadata copied to qemu-s9/backend-build-067-ubsan. Both compiled fan-qtest002-ubsan and fan-empty-qtest002-ubsan pass.
- C fan ABI first attempt failed because LeakSanitizer cannot run under this environment's tracing. Failure retained in fan-ffi067.log. AddressSanitizer and UBSan then PASS with ASAN_OPTIONS=detect_leaks=0, explicit existing environment limit; this is not a leak-check pass. Binary and successful log retained under checks-2026-10-21-001.
- ACTIVE original036 uses backend067 diagnostic-only C changes, same035 host/pool client behavior and all full mining/storage/dashboard options. Current exec session must be read from the tool response; output qemu-s9/sd2022-private-pool-036. Next: inspect exact fan failure details if it stops; if full execution passes, audit fresh mining and Core for this identity, then diagnose/reproduce the intermittent035 defect before claiming resolution. Do not treat a non-reproducing run as fixing035. Continue remaining native configuration/browser/WebSocket/remote gates after the fan investigation.
- Current original036 session is85490. Its console reached sample30 and completed; no fan diagnostic error has appeared. Wait for its final result/pool completion before auditing. A pass will not resolve the intermittent035 failure.
- Additional isolated clock inspection002 succeeded. TCG MMIO helpers use BQL_LOCK_GUARD. system/cpus.c SHA bf3692c8c7a0e6122f696ab4e06a6167971e78b693ffb5cc44ad0213aece375b selects cpu_get_clock unless an accelerator override exists; TCG only installs icount_get when icount is enabled. This run does not enable icount. The source evidence does not prove the failure cause. Diagnostic067 still preserves the original fail behavior.
- Original036 completed PASSexited0 in237333ms, no trace truncation. Native HTTP probe PASS11requests; full pool pcap547521B retained; native sync and SD snapshot complete. Independent regtest-audit001PASS253731computed hashes/251complete ASIC serial replies/two fresh jobs/clean transition/difficulty change. Core0 and1 both PASS acceptance and restart persistence. Block hashes09d1be4e026852cc0aa989799c3ccfa3e4346d06ebe3c73f40c189b8df38f79e and033ed7fa03b868b60d53c67492f5a16238e50dff42d3b68a470a2ae8631a7ecd. The old035 intermittent fan error is STILL OPEN;067 only adds diagnostics. No complete exact target/full dashboard gate.
- Added NATIVE-BOSER.md and README link, with actual036command, pinned firmware/asset identities, isolated local VM topology, passwordless-default behavior and open capabilities. Guide link check original036-links001PASS. The external publishing/SSH/TLS path is not claimed for original036.
- ACTIVE diagnostic reproduction runs037/038, exec sessions36862 and97253 respectively. Both use067, fresh independent pool work, full native HTTP/storage/mining options and separate run-owned state. Do not reuse036results. Next inspect any `Fan clock moved backwards` or `Fan advance rejected` message. If these pass, the intermittent035 cause is still not established. Consider a controlled host scheduler-delay test for the fan's60second-per-call bound, preserving elapsed time; do not silently clamp time or duty. Full source hashes freeze before future acceptance runs.
- Both diagnostic repeats037/038 completed exited0 without trace truncation or fan error. Their final mining audits run under sessions99000/49849; check output before Core claims. They do not close the intermittent035 defect.
- Added Rust s9_fan_pause_probe BACKEND NEW_OUTPUT 0|65. It reuses project-owned CPU regression fixture c1f18e0510661f2ed6c0679233b74a48d7641fd67cbfa99cc27c15952e7433de from cpu-regression-prep005, never archived miner firmware. Qtest controls remain private stdio inside the board namespace. Actual TCG CPU fixture must emit ABCDERP before fan writes/readback. Baseline001 completes and reads100RPS before/after. The65second variant stops only its owned QEMU child with SIGSTOP, waits for WUNTRACED, then resumes with SIGCONT and reads actual fan state. This diagnoses scheduler delay; it does not adjust guest clocks or count as firmware evidence. Its result is explicitly an observation, not conformance. Clippy001 caught unused import/shared guard function; fixed, Clippy002/build002PASS.
- ACTIVE fan-pause-delayed001 exec84071. Wait for actual diagnostic result and inspect the backend field. A reproduced advance bound failure may explain a class of failure, but does not prove035 had that cause without matching clock evidence.
- Cleanup030 uses KEEP canonical036input-store. It reclaims only duplicate bytes from finished035/037/038. Check completed journal and count; keep canonicals030/033 too. Root currently has about2GiB free; /tmp about12GiB used. Original archive and unique run artifacts remain intact.
- Core037/038 both candidates PASS: accepted=true, height0→1, restartheight1 and persisted_after_restart=true. Exact hashes remain in each block-acceptance.json. Cleanup030 completed128objects/570014396B, canonical036 preserved. Agents remain errored at unchanged account usage limit; no independent-review pass.
- Controlled fan-pause-delayed001 reproduced the late-callback defect on067. Baseline100RPS becomes0 after a65second owned-child SIGSTOP. Diagnostic now67410945965, previous2410816766, elapsed65000129199ns; the bounded model rejects elapsed>60seconds. This establishes a reproducible defect, but does not prove original035 had the same cause because035 lacks elapsed diagnostics.
- Implemented fan advance_elapsed to retain all elapsed nanoseconds through bounded advance calls. QEMU now calls this wrapper. No clock rescale, tach fixture, changed motor parameter or time clamp. Existing bounded API rejection preserved. Added analytic pulse-count/coast-down/partial-window test and C ABI delayed-timer case. Next run tests, freeze068, UBSan backend build, C ABI sanitizers, real65second pause regression, then new original firmware/mining/storage/dashboard run and its independent/Core audits. Also added implemented dashboard-probe to CLI usage.
- fan-elapsed-tests001 PASS7, Clippy001PASS, fmt001PASS, C ABI068 ASan/UBSanPASS (leak detection remains unavailable under tracer). Frozen068316files, libraryd6b6c4262a451beb3de8c1fb0171ee6dd688d06ed05e14d0eb05d8f64bd4ef0b/32054362B.
- Backend068 FAILED at link,178573ms. Undefined md_fan_advance_elapsed. Root cause: builder reads mutable work/simulator/qemu-s9/libmd_sim_qemu_core.a, still066library89fca75..., while checkpoint068 only retains the newly built library in its checkpoint. No backend068 executable produced; retain failed metadata. This is a preparation error, not a fan-model pass.
- Builder now requires trailing core=CHECKPOINT_DIRECTORY, validates manifest library hash before preparation and again inside isolated build, captures manifest as input and records it in build report. Next freeze069 and use it explicitly for backend069. Historical README backend commands need update after successful new command.
- Added strict pause assessment to the diagnostic helper, preserving raw observation first. It now fails for bad fan readback, backend errors or an unobserved requested pause. Negative regression passes. Source freeze068 predates this helper assessment and dashboard schema query change.
- Native frontend isolated extraction014 (www/boser/898.1ead42121500322bb188.js) hash492046aac3cddae49b1f2b650df13e4594794aeca6d6755bf6433ac2be41fdcc/37124B. It uses storage.update(input:{webTheme,preferredLanguage}) and StorageData/PublicStorage; IoError is explicit. Original dashboard state has null values. New probe reads native input field/subscription introspection before implementing a write/restore test. New pool guest build native-schema001PASS. No configuration write pass yet.
- Full Rust workspace tests pass after the fan/builder/probe changes: fan-elapsed-workspace-tests001,88binaries/293tests/0failed/0ignored. Used CARGO_TARGET_DIR=/tmp/md-sim-checks-069 to avoid filling root storage. Strict Clippy002 and fmt002 pass. Invalid core checkpoint regression rejects before creating output (backend-core-rejection001).
- Backend069 ACTIVE exec24004, explicit core=work/simulator/qemu-s9/core-sources-069; same library hash as068. New checkpoint316files includes strict pause assessment and native schema query. After build require fan and fan-empty compiled tests,65second pause assessment, then original039 with all actual mining/storage/dashboard evidence.
- Cleanup031 attempt REJECTED without journal/deletion. Canonical/tmp/md-sim-qemu-backend-031 now resolves to /home/user/mnt/small/memory-relief-20261021/md-sim-qemu-backend-031/inputs. The existing reclaimer correctly refuses a canonical path outside its allowed roots. Do not modify external moved storage or weaken the path check. After069 finishes, use its complete local inputs as a new canonical if cleanup is needed. Keep all old canonical references.
- OriginalBoserbinary015 extracted through isolated reader:313a6c8bbec28689a7df5d7817ba1ce7c1bd1de510e5b5984193702b69b0da84/2908268B. It is UPX-packed; path search on packedbytes supplies no config evidence. Do not infer configuration semantics from absent strings. Use actual native schema/read/write tests.
- Backend069 UBSan and11smokesPASS. Binary3243e54454793adef73bdadc8a544db225c501332c589e67390269a44d1170c3/19879288B. Manifestdccc3037f81fc782fcdfc74088c04efb7c3a44764dc0cd4cfd0d4068fbc310e0. fan003/fan-empty003qtestsPASS. CPUtimerfirstcommand rejected already-used output (no execution); freshcpu-timer-qtest-backend069PASS. Metadata copied to qemu-s9/backend-build069-ubsan.
- fan-pause-delayed002 actualfan before/after100RPS and65000ms pause, but strict helper assessment FAILED due two expected no-peer warnings from this intentionally disconnected CPU/fan machine. Raw failure retained. Exact two warning lines are now allowlisted; all other QEMU diagnostics and anyMMIOerror still fail. Negativeassessmenttests002PASS. Freshfan-pause-delayed003PASS65000ms,100RPSbefore/after; no fan error. This closes reproduced latecallbackdefect, not proves035cause or physical calibration.
- Devcachecleanup004 removes only target/debug rebuildablecache,3473files/2.6GiB. Release/capturedtestinputs and raw results preserved; workspace tests293PASS already recorded fromseparate/tmp/md-sim-checks069. Root3.2GiB free afterward.
- Cleanup031 secondattempt succeeded using NEW KEEP canonical /tmp/md-sim-qemu-backend-069-ubsan/inputs. Reclaimed121duplicateobjects/1396803124B from067/068; journalpaths and restorecopiesretained. Do not reclaim069canonical. Externalmoved031 untouched.
- Original039 completed exited0,237280ms,no trace truncation. NativeHTTP11requestsPASS plusactualGraphQLschema: StorageIn nullable fields webTheme/preferredLanguage/dashboardDensity; StorageMutation andSubscriptionRoot haveactualnativeinterfaces. Audit001ACTIVE. Newrun040ACTIVE with nativeAPI settings update/read/restore/read through originalBoser. This is15HTTPrequests; restoration checked against original nullvalues. A nativewrite response alone is not a pass. Freshpoolwork remainsrequired. Next inspect040configuration result; ifrestorefails, investigateactualnullsemantics anddo notweakenassertion.
- READMEbuildcommand updated to actuallytested explicitcore069 workflow; historicalS9IOdocs retain oldtestartifactcommands andlinkcurrentbuild instead ofobsoleteimplicitlibrarybuild. Guidecheckafternewdocs stillneeded. Strictprobeassessment/configwrite changespostdate frozen069; theirrunELFinputhashes arecaptured, futurecomplete sourcefreeze stillrequired.
- Original039 independentauditPASS275437hashes/248completeASICreplies/twofreshjobs/clean+difficulty. CorebothPASS andheight1persisted:049c9325039893a5a324a986b799b7930e4dbb84b8ca8a17f487665102257418 and02605340009d99dadeb17d1aade64f3b2c751c593deae09ce42318ca060a022d.
- Original040completedexited0,no truncation. Native15HTTPrequestsPASS: unchangedactualassets/login/uptime plus storage.update changedwebTheme null→DARK, independentreadDARK, updatebacknull, independentreadnull. Otherpublicvaluesremainnull. AuditPASS240352hashes/262serialreplies/twofreshjobs/clean+difficulty. CorebothPASSheight1afterrestart:0568dcf803d85b9c136a5b078b5d326be9f710d2b830b197d9bff9b0db65bff9 and0608daa101c3f09061899cb940be38758907a4b5b17549f853bdf76331de48c7.
- Addedstrictversion1native-stateplan and dashboard-state=PLAN_JSON. Writephase learnsactualoriginalvalues, changesonlytheme; restorephase requireschangedvaluesafternewboot thenrestoresoriginal; verifyphase requiresrestoredvaluesafteranothernewboot. Parentcapturesplaninsideclientimage andconsolemetadatahashesactualcapturedplan. New s9_dashboard_cycle orchestrates3freshboot/clientVMpairs, requiresoriginalSPL/UBoot/kernelmarkers, sync/snapshotlink, nativeAPIphase, independentminingaudit andtwoCoreacceptancesperphase. Noexactphysicaltargetclaim. Frozen071318files. Fullworkspacechecks native-state001PASS89binaries/298tests/0fail/0ignored, strictClippyPASS.
- Cycle001FAILED inphase0, beforeanyrestart. Nativewritecomponentpassed null→LIGHT, butguestscriptendedafter sample29 with no completion. OverallProcessResult exited1,340576ms,no truncation; snapshotguest_sync_completedfalse. BackendhasnoFanerror; finalstatusSIGKILL is ambiguous because SerialGuest kills the child after an incomplete script. Reported waited-child CPU305509459user+14616795systemus over333305ms includes supportchildren; doNOTclaim CPUexhaustion from this alone. Preservefailedassessment/rawlogs/snapshot; nofullminingaudit/Coreacceptanceforcycle001. Itsdriverbinary retained b8d50693551fc610a068cd0799464a9827e404d461f172f6d0d3e3d342d489ba/1190608B.
- Added pre-cleanup diagnostic to SerialGuest: childtry_wait status, readerror, andownedchild /proc stat/status/limits BEFORE our SIGKILL. Nextreproductioncanseparatepriorbackendexitfromcleanupkill. No timing/cap change.
- Reviewalsofoundindependentcycledefect: originalconsole014 requiresabsent/etc/bosminer.toml, so itcannotrununchangedonrestoredSD. Newdriverderivesseparateresumeconsole viaexactboundedreplacements: permitsalreadyrunningnativebosminer, comparesstoredminingconfigagainstexpectedbytes, validatesstoredstaticnetworksettingswithoutrewriting/restartingnetwork. Firstconsole014remainsbyteexact. Derivedscriptandparenthashesretained. Nextbuild003/test002thenfreeze072andcycle002. Do notclaimthisunexecutedrestartfixpassesyet.
- ACTIVE cleanup032 deduplicatesfinished039/040inputsagainstKEEP036;checkjournalcompletionbeforecounts. Allcanonicals030/033/036and069build remainprotected. Source/checkpoint070recordsHTTProundtripimplementationbeforestateplans;071recordsfirststatecyclebeforelatestdiagnostics/restartscriptfix. READMEcurrentbuildusesexplicitcore069;FAN.mdlinksreproducedfailure001 andpassingpause003. NATIVE-BOSER.mdstillneedscurrent040write/read/restorationdetailandfinalcycleevidencewhenavailable.
- Latestrestartconsolefix andprecleanupdiagnosticbuild003/004, strictClippy003 andtargetedtests003PASS. Test002filteredserial_console tests; test003runsallselectedexamples. Frozen072318sourcefiles. ACTIVE cycle002session13184 withsame069backend/pool003, unchangedcaps330CPU/360wall, correctedseparateresumeconsoleandfailurediagnostics. Do not rebuild its companions s9_boot_bundle,s9_pool_guest,isolated_network_vm,s9_asic_audit,node_prerequisite_probe whilethecycleisrunning; it hashes them before eachphaseandatend. Currentnewdriveralso retainsitsownbinary. No frontendbrowserimplementationadded yet; onlyreviewedexistingcapturedFirefox/runtimeinterfacesfornextUIwork.
- Cleanup032completed82objects/348207160B from039/040→KEEP036. Originaluniqueinputs/resultsunchanged. CurrentNATIVE-BOSERdoclinks040native15requestAPIroundtrip, independent240352hash/262replyauditandbothCorepasses. Restartcycle001failureisstillfailure; cycle002mustcompleteall3phasesand6Coretests.
- Cycle002 FAILED at phase1. Phase0 completed in237377ms, native theme null→DARK, clean SD sync, independent mining audit and both Core checks passed. Its blocks are01e512d300fb0180db9535b660d7e39cff07c7ccb17adbf72b5ce24dff163e87 and0525bc9000d14f03431e65c80c3501ba671a1bc72982d44cb05d90e846d20338. Phase1 booted original SPL/UBoot/Linux but saved network settings became DHCP; the strict restart console failed. Its dashboard could not connect. No restart pass.
- Exact isolated native source extraction016 proves preinit generates a fresh local MAC if NAND/factory/uEnv identity is absent. Extraction017 proves native SD cleanup removes saved network/system configuration when MAC or uEnv content changes. This explains cycle002; do not hide it by reconfiguring every restart. Original uEnv is empty. Planned fix: a separate native provisioning boot writes a recorded local MAC to the run-owned uEnv, then the configuration cycle starts on the next boot. Original boot code and archive stay unchanged. This is an explicit environment adaptation, not factory identity or physical-board proof.
- Core cleanup001 completed79duplicateinputs/530832920B. NEW KEEP canonical work/simulator/bitcoin-core/original-s9-030-candidate-0/input-store. Restore removed objects with copies from the journal; all unique evidence remains. Three review agents still report the unchanged account quota error; independent review remains open.
- Native network provisioning implemented in s9_dashboard_cycle. It adds a separate native boot that requires empty archived uEnv, writes only ethaddr with a fresh local unicast MAC, verifies SHA256 and syncs SD. Three test phases then require that exact MAC and uEnv. Native .last_mac/.last_uenv files are never edited by the driver; normal preinit cleanup runs before the first tested network configuration. Cycle003 provisioning PASS27760ms; MAC52:56:a7:c0:b0:77. Independent saved-SD inspection003 confirms unchanged original boot.bin, u-boot.img, fit.itb, system.bit.gz and derived26-byte uEnv SHA4ef8781fcd6da59e5cdc1bc59c7168172819e71ac9f3e785d79feee719267296.
- Cycle003 phase0 FAILED349111ms, exited1, no trace truncation. Stable native MAC check and HTTP state write passed, but serial stopped at sample12 and ASIC traces stopped at113.792418575virtual seconds. The supervisor observed no completed script and no already-waitable emulator exit before cleanup. Waited-child CPU327716847user+2142198system microseconds over341645ms is near330second CPU ceiling, but includes supportchildren. Preserve failure; no complete mining/Core gate for003. Do not raise caps or infer that the MAC fix caused the stall.
- Added slow S9IO catch-up diagnostics only: after4096 binding calls, report prolonged advancement every1second without skipping clocks or changing results. Frozen073318files. Backend073 UBSan build PASS185125ms, binary765cb92e893a1488d911e2fbde9326b554b634787b6d4c1987d558a0d231722d/19884024B, Rustlibrary0f861212c5192e70473c6f7c38bb054a1867d2ba1622ac9f1157f2d0e9d5c9c1/32155374B. Metadata copied to backend-build073-ubsan; full result remains /tmp/md-sim-qemu-backend-073-ubsan. ACTIVE qtests s9io-v1 session55219, bm1387-temperature session96216, fullworkspace tests session48194. Next inspect tests, then new original mining run with backend073 and cycle003 provision snapshot to reproduce the stall with diagnostics.
- Exact source inspection019 adds archived QEMU GEM, TTC, network socket and timer dispatch files. Existing TTC deadline patch already fixes zero-time callbacks and disabled-timer cancellation; do not redo that completed fix. Current stall cause remains unresolved. S9IO model catch-up, other timer loops and upstream network paths are hypotheses, not findings.
- SD inspector now independently reads native web_storage, network and .last_mac/.last_uenv, plus all5boot members via captured qemu-img and7zip inside preparation isolation. It deletes only its temporary ext4 copy before reading FAT to keep96MiB scratch bound. Inspection002 confirms cycle002 phase0 actually stored Dark and static10.0.2.15. Inspection003 confirms provisioning changed only observed uEnv among boot members. These inspections do not establish complete overlay executable integrity or exact-board identity.
- Core cleanup002 completed91duplicate source-evidence copies/1306380127B. Reclaimer verifies each against source-run-link object hashes and original local run files, rejects linked copy directories/files, and journals exact restore sources. KEEP all original runs030/033/036/037/038/039/040 (their original logs/results are canonical). Original archive and unique evidence untouched. Rebuild/check logs core-evidence-cleanup-build002 and native-state-inspect-clippy003 pass.

- Continued 2026-10-21 at12:25UTC. All3review agents still account-quota blocked until25Oct08:48; no unchanged retry. Zero complete exact targets; no release pass.
- Original041 PASS237350ms on ARM073/pool003, archived2022SD, cycle003 provision snapshot, native stableMAC52:56:a7:c0:b0:77, native settings write/null→LIGHT, clean SD export. Independent audit211492computedhashes/201serialreplies;2freshjobs/clean/difficulty. Core candidate0 block05523b8f03dbc4e378cca4115f1bb92d7c9fa0fbff6cd38ae61f9ea321d4132a and candidate1 block06a3654303a7b9f1b3f28cd526866004d6ccfc5a7bdc8d198f0cff83a00a1493 accepted, height0→1, persisted after restart. No complete dashboard restart inference.
- ARM074 diagnostic backend PASS188477ms/11UBSan smokes, binary5342688fcaf4e6f8172ec3637299422059d197b0371b65c1cd50d7e3e38350d6/19879448B. It adds timer-dispatch diagnostics only after65536callbacks, unchanged deadlines. Timerpatch7b3da99c1c353e3b046fc5fa2cb212ebcfc9a8bed7c7368c86d53a3b01fe0468. ARM075 PASS11smokes, binary289efdb849772b94f0cc009a79fc1e71e23affc9dc4240b8a3808f0f1b358baf/19888568B, captures pre-strip nm symbol map. 074 and075 binaries differ; do not use075 offsets to label074. Both Rustlibrary0f861212c5192e70473c6f7c38bb054a1867d2ba1622ac9f1157f2d0e9d5c9c1. Compact metadata underbackend-build074/075-ubsan; completebuildresults retainedin/tmp.
- Cycle004 FAILED in phase1restore. Provision/nativeuEnvethaddr96:83:9b:1b:0d:58 SHAef3f898307294fbb4866851988ec397782c320d929c7e050a055c97fc63ed126 passed. Phase0write passed with two Core blocks0fe9ccbc012c3576c886c61d32ea7feaf4e2ee1c3a340748420217c1270fbff5 and05e9c86e02131be9f7832ed82c6f86c098ee9b9d39f8956993993b9402579dae, each persisted after restart. Phase1 actualguest verified sameMAC and persisted staticnetwork. It then stalled. No phase2. Preserve failedassessment.
- Critical004 diagnostic: slow_advance link8 first now95863214065,last94836781825,calls8192,idle0,status0x9. Later now128228084605,last103412956865,calls368640, each4096calls advancesonly83886080ns (4096edges/call,5nsedge). No outgoingdeviceevents acrossmillionsofclocks. This identifies S9IO catch-up as the stall path, not a generic QEMUtimerloop. Need identify subcomponent before claimingrootcause. TX incomplete work and incomplete command waits have no idle optimization; current test explicitly expects <=1idle forpartialwork. These are hypotheses pending detailedtrace. Do not skip time, raise CPU budgets or manufactureevents.
- Added read-only per-component idle diagnostics (receive/transmit/UART/ASIC) through Rust/C ABI. Behavior still unchanged. ACTIVE corefreeze076 session92969. After frozenlibrary, build backend076UBSan, rerun failed phase1 from004/run0snapshot andplan1 using console-resume.sh path fromcycle004 (inspectexactfilename). Preserveallfailures. Meanwhile implement clock-equivalent idle optimization only with slowclock and literalpacket regressionoracles.
- NewSerialGuest diagnostic polls try_wait up to50ms after script failure/EOF before ownSIGKILL; records elapsed observation. Targeted5testsPASS, workspaceClippy timer-symbol-clippy002PASS. Thiswasafter075freeze andnotin cycle004runner. Rebuildrunnerafter076freeze; noactivecyclecompanionconflictnow. Fullworkspace tests earlier native-network001PASS89binaries298tests. Need fullchecksafterlatestedits.
- Reclaim033 removed196cycle001/002/003CAScopies833565001B toKEEP036inputstore;034 removed04140CAS174100737B toKEEP036. Core003 removedcycle00226copies287135368B;Core004 removed04126copies263654756B, canonicalKEEPoriginal-s9-030-candidate0/input-store + originalsource-runlogs. ACTIVE cleanup035cycle004stores andcore005cycle004Corecopies session37647. Journalsgive COPY restoration; nohardlinks. Alloriginalarchives/uniqueresults preserved.
- Next: diagnose and fix S9IO idle catch-up, then repeat complete3phase settings cycle includingeachphase2Coreblocks. Continue originalnativebrowser,remote,exactmatrixgates afterward. Do not finish at this milestone.

- Correction: the preceding continuation entry was written around12:20UTC, not12:25. At12:34UTC cycle005 is active, session49548. Do not rebuild its five companion binaries until it finishes (s9_boot_bundle,s9_pool_guest,isolated_network_vm,s9_asic_audit,node_prerequisite_probe). Source edits and separate dev checks are allowed. Full user task remains open; no final.
- Core076 diagnostic-only freeze319sources/librarybe26a9f49cb4a546543edef4191a44695438d2b84382e782c9ae51b5c904ac01/32162330B. ARM076UBSan build PASS184022ms/11smokes, binary2ca4e1905f276dda2c73777271d00c8952a1df065ea585581f65c4ad9b78d329/19888408B. New read-only S9Idle CABI reports separateRX/TX/UART/ASIC clock bounds; original advance logicunchanged. Actual042 reused004phase0snapshot andplan1Restore; PASSnative stableMAC/staticnetwork/restoration/sync. No stalldiagnostic in042. Independent042audit262012computedhashes/276serialresults/2freshjobs/clean/difficulty. Core042candidate0 accepted0278cf200109b4e97e2a990115931f98fbf22cdc0281c67d682f5f25c5b6aa87;candidate1 accepted03127f9a0160390afef05dc69f9e4c46cabc1e5497d131c4a67e764e478e290a; bothpersistedheight0→1afterrestart. Thisrepeatdoesnotcomplete3phasecycle.
- Found two clock-polling defects in S9IO TX: incompletework loops Wait/Check, and incomplete9bytecommand stalls CommandData awaitingsecondFIFOword. New pairedslowclock+fastclock regressions BOTHFAILbeforefix(partial-fifo-regression-before001). Newoptimization retains allFIFOwords, free-timer/ACK/statephase, serialevents, pendingresets andlive midstate thresholds. Partialworkpolling uses currentmidstates; missingwordstates skip onlywhileCRCready andenabled, UARTboundsremain. No guestclock/hashrate/filter/noncechanges.
- Replaced old incompletework test that asserted an implementationdetail(idle<=1) with stronger no-prefix/retaineddata/literalarchivedpacket/clock-by-clock regressionoracles. Testscover11ninebytecommands,one/fourmidstatework,4timerreloads,modechanges,pendingqueue resets,u64spanphase. AddedCABI200millionclocks(1s200MHz) singleadvance duringpausedcommand thenliteralwirebytes. FirstCABItest001failedbecausetest droveRXDlow(BREAK) whileusingnopartner; correctedhelpertoexplicitRXDhighforstandaloneinterface;existingpeer testskeepfalseinput. ABItests002PASS6tests. S9idleafter002PASS13tests. CABIstaticlayout anddiagnosticread-onlychecks pass ASan/UBSan(partial-fifo-ffi-sanitizers001;LSandisabledexistingtracinglimit).
- Frozen077319sources,library219d99045c480a220d6a0d26df387365db6ac450cee3a7ea24a1c5a3c37877dc/32162454B,manifestsha a4ed73df98dd64cc5e83b0d7d258e10c12bf9355405c74e12efd66f38f0e787d. ARM077UBSan build PASSbinaryda8450ba666f4c753f699ce9d9c93b539a1026ba20343388ffe5679ae7f97a75/19879480B. Compactmetadata076/077savedunderwork/simulator/qemu-s9/backend-build-NNN-ubsan.
- Added deterministic compiledqtest: actualMMIO first9bytecommandword,1virtualsecondpause,secondword,completepacket,existingquery+receive/error/reset suite. Old076FAILexit137CPUboundafter24responsesof1396; fullfailedlogs preservedpartial-fifo-before076. New077PASS1396responses+18IRQtransitions,actualwirebytes/intervals. TightenedverifiertorejectTXprefixbeforesecondMMIOword; repeated077-002PASS. Native004stallwas inS9IOcatchup; exactstalledTXsubstatewasnotcaptured,so doNOTassertabsoluteidentificationofthatintermittentcase solelyfromdeterministicrepro. Actualfullcyclemustpassandmorefailuresremainrealdefects.
- Cycle005command: target/release/examples/s9_dashboard_cycle work/simulator/original-zynq/sd-bundle-006 /tmp/md-sim-qemu-backend-077-ubsan /tmp/md-sim-qemu-pool-003-ubsan work/simulator/qemu-s9/original-dashboard-cycle-005. ProvisionPASSMAC9a:4f:a2:27:a3:67;phase0reachedsample30at12:34UTC;no slowadvance. Awaitaudit/Core0thenrestorephase1+Corethenverifyphase2+Core. Currentcompanionbuildpartial-fifo-tools-build001includes50msSerialGuestexitdiagnostic.
- AllcurrentchecksPASS: workspacepartial-fifo-workspace-tests00289binaries301tests0failed0ignored;workspaceClippy002;fmt002;guide387links;compiledBM1387temperatureqtestbackend077PASS. Originalnativebrowser/remoteexactidentity/fullmatrixstillopen. S9IO.md nowdescribespartialqueueadvancementandlinksoldfail/newpass. No independentreviewagentavailable.
- Cleanup035cycle004114CASduplicates485357841B→KEEP036inputstore;Core005cycle00426duplicates288854700B→KEEP030CoreCAS+original004run0logs. Newcoordinatorcleanup extension onlyaccepts strictqueueNNN/input-store orworkerNNN/store underwork/simulator/coordinator, requiresfinishedqueueattempts/workercleanup,holdsStoreexclusiveleases,usespersistedlimits. Firstbatchcorrectlyrejectedaworkerwithoutcleanup;nojournal/deletion. SecondbatchPASS276duplicates4646038977B→NEW KEEP work/simulator/coordinator/queue-007/input-store. Journalwork/simulator/coordinator/duplicate-input-cleanup-002.json (logattempt003). Reclaimedqueue001/003/004/005/006andworker007copiesonly. Worker003/004/006preservedbecausecleanupstatusnotallverified. COPYrestorationbefore rerunningoldqueues. Root4.7GiBfreeat12:34, /tmp29GiBfree. Originaldownloads/uniqueresultsunchanged.
- Nextbrowserwork researchedbutNOTimplemented: extendprivatepoolVM withcapturedFirefoxpayload, separatebrowserUID1001(root_setupeth0/lo)beforepoolUID1000drop. WarmbrowserBEFOREMD_SIM_POOL_READY sooriginalfirmwareclock/230spooldeadlineunchanged; separateboundedstartupCPU/wallneeded,explicit2GiBsupportVMandchildminer330CPUlimits. Nativeapp011exactSHA1c318... haslogin-username/login-password, /graphql, ws://selectedhost/graphql, eventsubscription. NeedinspectactualWSprotocol/nativeDOM, actualbrowserlogin/liveasset/telemetry/settings/WS, thenSSH/TLSsameactualrun. Existingnative024browserusesdifferentadaptedLuCIidentity,cannotreusepass. Readsupport/pool_vm.rs, browser_driver/environment/groups and s9_pool_guest.rs. No codeyetfornewBoserbrowser.

- 2026-10-21 13:10 UTC continuation: task remains incomplete. Frozen matrix v11 still has 79,599 rows and zero exact-target passes. No final. Agents remain quota-blocked until Oct25 08:48AM; list_agents confirmed all three unchanged. Independent review gate remains open.
- Cycle005 failed its final HTTP probe after actual firmware script completion (237218ms): Boser opened TCP then paused accepts during startup. The first GET timed out before it resumed. Write/restore phases and four Core acceptances passed; retain final failure. Initial HTTP readiness now permits at most four GET / transport retries within40s, only curl7/28 with HTTP000; HTTP errors and all later requests do not retry. Exact index2044B/SHA d0d769811cbf2e5290dafb5f9f9a005bcafd45469e94899080353093e1f20947 required. HTTP cookie directory0700.
- Cycle006 PASSED all three linked native settings boots: write DARK, restore originalnull after restart, verify originalnull after second restart. Frozen copies in work/simulator/cycle-tools-006/SHA256SUMS; backend077 and pool003. Command: work/simulator/cycle-tools-006/s9_dashboard_cycle work/simulator/original-zynq/sd-bundle-006 /tmp/md-sim-qemu-backend-077-ubsan /tmp/md-sim-qemu-pool-003-ubsan work/simulator/qemu-s9/original-dashboard-cycle-006. assessment.json passedtrue, scope only nativeAPI/settings persistence plus perphase freshwork/Core. Original-board/full dashboard/exact matrix remain open. Completedprogress binds audits,snapshots,execution IDs and six Core results. All six accepted and retained after node restart: write02fc9c660201e476d8adbf3de88d9c9962b8dd5a2f352f56957875e045785be3,042122bc039a45eda7e628a16b96e291a3b09b36a59007b45b89a4b4510c12df; restore03d50045031ce216f4858a301c16f33758270e2e8fd47df722a52f5a567fdc3e,04dec56203dd71c1bd5c18ff8bb93a5d5d63979dbedbbd0a15a3dc4a4b2af178; verify0be7fcf10056c895e50be43d142e447b82c10b109eeed7e9b3a213cf5a835425,05a9f1bc0224b5119f71c765a1e3418c88a603f34cb357bf653254289e607d8b. README and NATIVE-BOSER updated. Earlier intermittent fan035 and exactstalled004TXstate remain unresolved, not erased by repeatpass.
- New Rust originalBoser browser path implemented: s9_browser_check uses captured Firefox UID1001/newprofile actualDOMlogin, exactassetfetch, advancingHTTPuptime and graphql-transport-ws subscription. Exactvendorasset a7973f69f122bc511257c179788a2e74fef72010e836a4e19cc076a2c5d78435/1588210B extractedoriginal-boser-vendors019. RootonlyprivateVM s9_browser_pool supervisor serializes childJSON and stops/reaps separateUID1000pool/1001browser. Browser warmup before poolready/minerstart. HTTPprobe must finish logout before browseropenssession. Browser2GiB/2CPU support VM startup330s/600CPU; miningCPU lowers330afterVMspawn and ownwalltimer enforces original30..360s. Real30schildSIGKILL regression passed. Packetlimit16MiBbrowser,normal1MiB. No firmware/hashrate/filter/busclock changes. HTTP/nativebrowser usesselected10.0.2.15origin, no hostlistener.
- Browser043 FAILED PREPARATION ONLY, no firmware executed. pool_vm::capture incorrectly verified/mounted baseinitramfs while prepared-image identifies combinedbrowserimage. BaseSHA8ffbd9b62ee944f6d04227ba04646b38dbc65c73a2341831b3bf1dd763d8fcf2/8287218B; payload ecad20133f7e5827ae2995fbbe14ba5f5917f42940a23ed430cda4413326c2dc/406343028B; combined2399a25b0cbb038959f27ffd80528238c43f55455f2381b07674d7c4b31d5d8d/414630246B; rootmanifest7146b6d336a580e4ff9b61110ed900f04fa0f7df7c0df591d8f8e57ba9cfa53d/858253B. Fixedcapture explicitbrowserbool selects combinedandverifies recordedhash; normalpathunchanged. CLIusageoptionadded. LoginDOM now boundedrenderwait before element lookup and authenticatedbody assertion. Build005active session2725; clippy007PASS. Run044next in /tmp to avoid rootdiskpressure; recordactualpath.
- Core078 sourcecheckpoint only:322sources, librarybb68fc82f88dd6cb80f946687a4cd53ec1a17cfcf6c513de507652084f7dc898/32162378B. No078QEMUbuild; selected077backend remains pinned to077library. Latestcapture/DOMfixes after078freeze, must freeze/check later. Browserworkspace006PASS90binaries303tests0failed0ignored, strictClippy006PASS, fmt006PASS,387guidelinksPASS. Newcapture/DOMchanges need affectedtestandactualbrowserrun. Independentreview unavailable.
- Cleanup037 cycle006154duplicateinputs659458578B→KEEP036inputstore, Core007sixchecks78duplicates836078784B→KEEP030Coreinputstore+original006runlogs. Restore by COPY, neverhardlink. Root2.3GiBfree and/tmp29GiBfree13:10UTC. Uniqueevidence/originalarchives untouched.
- Storageconcern: old KEEP work/simulator/network-vm/native-013/payload.cpio.gz,baseandcombined nowabsent. Directorycontainsoldreports only; cause not established. Do not say this turn deleted them. Exact oldpayloadreportSHA matchesnew043payload ecad2013... KEEP new043payload as knownactualcopy. Oldconcatcleanupjournals named in previoussummary were not found in bounded filename search. Browser003sourceCAS remains. Investigate relocation metadata before restoring oldreferences. Do not reclaimnew043payload unless another matchingcopy verified.
- 13:12UTC correction to storage concern: old canonicalpayload was /tmp/md-sim-native-network-013/payload.cpio.gz, NOT the compact report directory work/simulator/network-vm/native-013. /tmp path is externally relocated symlink to /home/user/mnt/small/memory-relief-20261021/md-sim-native-network-013. File exists, freshly hashed ecad20133f7e5827ae2995fbbe14ba5f5917f42940a23ed430cda4413326c2dc/406343028B. Base44513749B alsoexists. Combined removed by documentedconcatcleanup. There is no missingcanonicalpayload defect; summary path was wrong. Preserveactualcanonicalfile.
- Browser044 ACTIVE session34290 actualoutput /tmp/md-sim-original-boser-044, rootlogchecks/boser-browser-original-044.log. Preparedcombinedimageverified, poolVMstarted, browserstartupongoing. Same077backend/003pool/10000Hscap/fixed4adaptation. Hostdisk2.3GiB prevented safe extra2GiBpreparationthere; output usesauthorized/tmp. Build005PASS, clippy007PASS, workspace007running48354. Noactivebinarydependency issueaftercapturedinputs.
- Independent006finalSDinspection PASS2832ms exit0/notruncated. Fulloutput /tmp/md-sim-s9-cycle-006-sd-inspect, compactresult/processspec/inspection copiedwork/simulator/qemu-s9/dashboard-sd-inspection-006. Confirms restoredweb_storage nullsettingsSHA058f68f4b34213aec4a82e77940af8aab78afacd88d611afd28d060ed5d0920d/106B, persistedMACf2:cd:e5:5e:93:fa, networkstatic10.0.2.15, exact unchangedboot.bin/FIT/UBoot/FPGA hashes. uEnv26B SHAecc526b0f1cb4556d7248a3310d0db78d0c34cd2536b0cb164f50078229a3f81. Nohostcrash/NANDpersistenceclaim.
- 13:25UTC: Browser044 failed after265102ms exit1/notruncated. ActualFirefoxstartup91559ms, archivefirmwareexecuted. Browser100sHTTPreadiness expired whileHTTPprobe stillactive; parent correctlyfailedsupportVM. No browser/mining/Corepass. Itslogs/result/inputstores retained /tmp/md-sim-original-boser-044; rootcommandlogboser-browser-original-044.log. Separate lifecycle review foundrunnerdroppedminerafterconsolebeforebrowsercompletion. Fixed: browserreadinessusesexisting230spoolinputbound; nofirmwaredeadlinechange. HTTPprobe sends smallSESSION_CLOSED record afteractualsuccessfullogout/rejection beforelargeasset-evidenceJSON. Browserparent writesreadyonlythen. All15HTTPoperationassertionsremain. Addedperrequestelapsed/controlobservations. Runner nowwaitsforselectedoriginbrowserpassedresult BEFORE native sync/drop/snapshot, withinoriginal360wall/330CPU. Tests reject missing/wrongorigin/failed/duplicate/contradictorybrowserresult. No boot/watchdog success fabricated.
- Browser045 ACTIVE session58061 /tmp/md-sim-original-boser-045 withsameexactarchive077backend003pool. Rootlogboser-browser-original-045.log. Build006PASS; s9bootSHA652b9f7a1530999908791fa205bfc431979920d30d3c2d02ad41f3cb1cf449f5, poolguest ea4a173e35898adcf335caf9a53370cc6e6ebff01064e56dc5cd6c9950bec5af, browserhelper cc8abdc89e200de057f4f56f7c0da04e1fd7539795b70c6b419fa34ccfc985f8. HTTP15actualrequestsPASS/sessionclosed, nativebrowserfetchesrealindex/app/vendors/css now. NeedactualDOMlogin/liveHTTP/WS pass, auditand2Core, keepfailedruns. No final.
- Workspace007PASS90binaries303tests0fail0ignored, fmt/strictClippy007/guide387PASS beforelifecyclefix. Lifecycle-targeted001PASS, strictClippy008PASS afterfix. ReclaimerextendedonlystrictbrowsercaptureNNNandfinishedpool-vm-image/input-store, verifiescapturemanifesthashes/preparationexitstatus/rootmanifest, parentterminalfailureorresult, exclusiveStorelocks andallindividualduplicatehashes. Tests+strictClippy009PASS. Reclaimerbuild001PASS. Cleanup browser-preparation-cleanup001 removed3503duplicatepayloadCASinputs915469497Bfrom043prep→KEEP work/simulator/browser/captured-003/input-store; images/uniquemetadata/originalsretained. 043preparation-failure.json bindsactualfailedcommandlog and statesno firmwareexecution. Root3.2GiBfree,/tmp25GiBfree. Correct006compactinspectionfilename is spec.json (copied), notprocess-spec.json; originalinspectioninput/result hashes intact.
- 13:44UTC: Browser045 FAILED297435ms exit1/trace_truncatedtrue after64MiB aggregate trace limit. It passed15nativeHTTPchecks, servedactualbrowserassets, but no completedbrowserresult retained. Firmwarekeptaliveasrequired; rawper-hashrecords grew beyondbound. Retainfailure /tmp/md-sim-original-boser-045. Noassertionortracepolicyweakened. Browser046repeatedwithexplicitpermittedlowerhash-cap-hps=1000 (defaultremains10000, topology/bus/filterunchanged). 046FAILED310106ms exit1/notruncated: testpoolclosedconnection aftertwoacceptedjobs; originalfirmwarereconnected until16sessionbudgetexhausted. Mainpoolstatesshowactualcomputedacceptedworkbutouterfailed; nobrowser/mining/Corepass. Retain/tmp/md-sim-original-boser-046. Browserresultwasnotprintedbeforeparentcorrectlystoppedfailedpool. 046backend28MiBfits64MiBtracebound.
- Fixedtestpoollifecycleinmd-sim-mining::stratum: CompletionPolicy CloseAfterWork(defaultoldfixtures) vs KeepOpen; newTCPandprivateinheritedwindowentrypointssharethesameprotocol/verifierloop. KeepOpencontinuestoindependentlyverifyfinaljob aftertwofreshacceptedjobs, preservesduplicate/stale/out-of-range/low-difficultychecks, endsatcleanpeercloseorplannedwindow. Metadatarecordsconnection_policy andconnection_end. Earlydeadline/cancellation/framing/resourceerrorsstillfail. Servernormalendnowjoinsper-sessionremainingdeadlinesinsteadofprematuresharedcancellation. S9poolusespersistentTCPmode,16connection/2048message/1024submission/512KiBpertraceboundsunchanged. OriginalfirmwareCPU330/wall360andpool230sunchanged. No manufacturedshares/timeouts.
- NewrealIPCregressionexercisesoneconnectionwithfreshnoncecomputation, twojobs+difficultychange, thirdacceptedheaderindependentlychecked, duplicate/stale/out-of-range/lowdiffaftercompletion, cleanEOF/windowend, partialframe,cancellationandbudgetfailures. Firsttestbuild001failedHeadernonCopymove;fixedclone; heldpeeropenforcancel/window/budgettoremoverace. Tests002PASS8tests1.08s; strictworkspaceClippy002PASS. S9submissionauditstillrequires completed,noerrors,noexhaustion,no timeout/no cancellation andtwofreshjobs. Allcomputedblockcandidatesarehardware-correlated; selectoneperdistinctjobfortwoCorechecks andretainadditional_block_candidates. Oldexacttwo-totalcandidateassumptionwouldrejectvalidpersistentwork, nowdistinctfreshjobassertionstronger. Old006run0replaytestACTIVE session(seedetailsrootlogpersistent-pool-audit-regression001).
- Browser047 ACTIVE session94705 /tmp/md-sim-original-boser-047, rootlogboser-browser-original047. Same077board/003pool/originalsd2022/archive/fixed4 adaptation; explicit1000H/scap. Newroottoolsbuildpersistent-pool-tools-build002PASS. Source079freeze322files/libbb68fc82f88dd6cb80f946687a4cd53ec1a17cfcf6c513de507652084f7dc898/32162378B existedBEFOREpersistentpoolandSDinventorychanges; newfreeze needed. No079QEMUbackendbuild. Workspace009PASS90binaries306tests0fail0ignored beforelatestchanges; latestpersistentworkspace001running. Noagentsavailablequotaunchanged;independentreviewgateopen. NO FINAL.
- ExtendedindependentSDreader inventoriesall45ext4entries,hashes24regularfiles,retainslinks/whiteoutsseparately. Exactregular-memberextractioncompareslistedsize/extractedbytes;512MiBprepRAM/128MiBtmp/64MiBfilelimits. Strictparserrejectsduplicatefields/paths,escapedpaths,typeconflicts. Actual006finalsnapshotinspection /tmp/md-sim-s9-cycle-006-inventory-002 PASS2952ms exit0/notruncated;compactfilesin work/simulator/qemu-s9/dashboard-sd-inventory-002. Stored nativeupdatebundle upper/usr/share/upgrade/firmware.tar.gz SHA37ade53b6181b69cab42f0e551cdd2957a7375b8e6eb276e9121a2560bb497a6/32090330B. DoesNOTassessOverlayFSextendedattributes orprovecompleteexecutableintegrity;scopeexplicit. Parsertests001/strictClippy001/fmt/guidePASS. READMEupdatedS9SPL/UBoot/services/miningreferencecapabilityanddiskinventory, exactphysicalgateopen.
- 13:53UTC: Browser047FAILED357960ms exit1/notruncated, butactualnativebrowserlogin/render/archivedasset/liveHTTP completed. ActualDOMtitleDashboard|BraiinsOS+,3hashboards6/7/8×63chips,65C/100MHz, actualpoolaccepted51/rejected40. DoNOTuseproduction-scalednativeGH/sdisplayasfunctionalrate; modelcap1000H/s andfixedfilteradaptationseparate. UIuptime4years isclockmismatch: originalguestwall2022, supportbrowser2026. Needrecordedclock/NTPenvironmentfix; no counterorclocksuccessfabricated. HTTPuptime194→212secondsadvancescorrectly. Browserinitialready92189msafterstartup. Oldrenderassertbody>100acceptedtransientexpired-sessionnotice beforefullUI; tightenednowrequiresactualHashBoards,PoolStatus,SystemStatussections. New auth.extend mustreturnVoidResult, notjustcookiepresence.
- 047failedatWebSocket: negotiatedgraphql-transport-ws butclosedbeforeanymessage. Archivedmainapp module30530 hasws.init = {headers:{"Accept-Language":window.i18n?.locale??"en"}}, whereashelper sent{}. Fixmatchesexactarchivedpayload (causenotyetproveduntilrerun), retainsclosecode/reason/wasClean/lifecycle. QuerynowrequiresBosminerInfoSample.timestamp andtwo distinct nonnullsampletimestamps, preservingrealWSdeliveryassertions. 047introspectionconfirmedtimestampfieldNonNullTimestamp pluspoolGroups,summary,workSolver,fans,tempCtrl. 047browserfullfailureDOM/logsretained inrootrunrecords. MainpersistentpoolPASSED completedtrue91submissions/endplanned-window-end;8separatebinaryprobesfailedexplicitly withzero submissions,notacceptedwork.16sessioncapnotexhausted,collection9completions;poolfunctionallifecyclefixworks. Whole047stillfailedandhasnoCoreclaim.
- Browserdiagnosticprogressmarkersaddedbefore/afterrealnavigation/login/assets/WS; diagnostic_onlytrue. FailureDOM/sessionclose now5sboundratherthan105snormalcommandallowance,ownedbrowsercgroupstillkilled/reaped. Build009ACTIVE session(fieldsrootlogboser-browser-ws-tools-build009) before048run. Currentclippy004PASSbeforeauth.extendlastaddition;rerunaffectedchecks. Source079nowhistorical.
- Persistentworkspace001PASS90binaries308tests0failed0ignored. Old006run0newauditorregressionPASS264770hashes259serialresults2freshjobs2blocks0additional. Exactoutputpersistent-auditor-regression001.json. NewCAScleanup002PASS10509duplicates2746408491Bfrom/tmpbrowser044/045/046prepstores→KEEPbrowsercaptured003CAS, onlystrictnumbered/tmp/md-sim-original-boser-NNN/pool-vm-image/input-store andcompletedparentresultallowed;test002andclippy003PASS. Alluniquereports/images/originalarchivespreserved. /tmp22GiBfree/root3.1GiB13:49UTC. Continue048browserfullrun,thenindependentaudit+twoCoreacceptancesonceactualpassed;SSH/TLS/originalexacttarget/fullmatrixstillopen. No final.
- 14:13 UTC: Original browser048 PASS, exit0,372388ms including Firefox startup88992ms, no trace truncation. Selected archived2022 firmware, backend077, pool003, explicit1000H/s and recorded4-bit filter. Actual Firefox renders Hash Boards/Pool Status/System Status, native auth.extend returns VoidResult, exact archived app bytes, HTTP uptime advances, WebSocket graphql-transport-ws delivers timestamps1651224441 and1651224442. Matching archived connection_init language headers fixed047's pre-ack closure. Native page still has2022/2026 clock mismatch and production-scaled rate display; full dashboard/physical-board gates remain incomplete.
- Independent048 audit PASS102121computed hashes/117complete serial replies/94hardware-correlated submissions.46accepted and48LowDifficulty, two distinct fresh jobs/clean transition/difficulty change. Select one candidate per job;44additional actual candidates remain retained, not all node-tested. Isolated Core candidates0/1 PASS acceptance and restart persistence, height0->1:03f32a2503168b9a257231899493c8103d08ec67d2c67351fbb4bf2b8443a24c and023d34790168e074947a7a1931ac7f1e731156245e232c7ea5d700b022a65b72. Compact evidence in work/simulator/qemu-s9/original-browser-048; full logs/result/packet capture remain /tmp/md-sim-original-boser-048, bound by execution-SHA256SUMS. Core output work/simulator/bitcoin-core/original-s9-048-candidate-{0,1}. README/NATIVE-BOSER updated; guide011PASS392links.
- Implemented opt-in adapt-clock-epoch=UNIX_SECONDS in s9_boot_bundle with support/s9_wall_clock.rs. Explicit epoch included in process environment/adaptation and support image contract. QEMU support RTC starts at fixed UTC with host clock. Original guest date sets realtime to epoch plus supervisor monotonic elapsed, before boser/bosminer start; original date must read back within5seconds. No monotonic/device/ASIC clock mutation, firmware byte change, physicalRTC or NTP claim. Browser compares native since+duration against actual browser HTTP send/receive interval, with5second whole-second/serial allowance. Epoch bounded before ARM2038. Unit UTC/leapdate/bounds tests PASS; clippy002PASS and workspace001PASS (counts pending). Actual049 ACTIVE session28685, same original path +epoch1792591709. Guest applied1792591854 after145seconds, readback exact; full browser test pending. Do not mark clock gate passed yet.
- Browser049 copied build s9-wall-clock-build002. After it started, browser introspection was extended to SummaryInfo/WorkSolverInfo/TempCtrlInfo for next full telemetry work; no model/counter adaptation. Build boser-browser-schema001PASS, not yet runtime-tested. Last full workspace s9-wall-clock001 predates this query-only change. Source079 predates persistent pool, inventory, wall clock and browser updates. Freeze080 needed after affected checks, not yet created.
- Cleanup browser-preparation003 PASS7006duplicates1830938994B from finished047/048 prepCAS -> KEEP work/simulator/browser/captured-003/input-store. Journals restore COPY only; no unique outputs removed. Current root2.7GiBfree,/tmp20GiBfree. All three agents remain errored by unchanged account quota (until Oct25 08:48), no repeated requests; independent review open. Next: finish049 clock/browser/audit/Core, extend native telemetry, then original SSH/TLS/remote; all full-matrix gates open. NO FINAL.
- 14:27 UTC: Original049 PASS372908ms, exit0/notruncated, browser250893ms after support readiness. Explicit wall-clock check passes: original guest readback1792591854; native since1792591836, duration202->236, request intervals retained. WS timestamps1792592076->1792592077. Audit PASS102301hashes/95serialreplies/74submissions (35accepted,39LowDifficulty),33additional candidates. Coreboth PASS0->1 and restart:0becc3f402a6cd8195e4d8ba00005a45d49456cebd747172a7fb934da6c21eef and0550e22e034b6efb08650bd67745753ff35becccbf1c7dba61e20a0d7b66e62a. Compact work/simulator/qemu-s9/original-browser-049, raw/tmp/md-sim-original-boser-049. README andNATIVE-BOSER include verifiedclockoption and limits.
- Original050 FAILED376833ms exit1/notruncated: extra introspection reached native-websocket-started258846ms, then support services260second bound stopped it before final browser result. NoCore/miningpass. Retain/tmp/md-sim-original-boser-050. Improved asset evidence transport: exact pinned UTF-8 JavaScript returns browser fetch text instead of425409JSONnumbers. Rust still verifies complete original byte size/SHA256. No assertion, clock, cap or deadline changed. Fullassetidentity remainsrequired.
- Implemented same-origin isolation using two fresh Firefox profiles, warmed before miner startup. Shared browser_driver now assigns2828+profile index and verifies actual moz:profile in NewSession. The second profile waits for first native auth.extend success, opens same exacthttp://10.0.2.15 origin, requiresnative login DOM/emptycookies/AuthError on native session extension. First profile must stillreturnVoidResult afterward. Separatecgroups/768MiB/256tasks perbrowser, sharedUIDlimit512, two2GiBVMcores remain unchanged. Both directbrowserchildren are terminated/reaped before either cgroup reaps adoptedhelpers. No newhostlisteners. Scope stillpartial; fulltelemetry/uploads/SSH/TLS open.
- Original051 ACTIVE session71866, same049 clock/firmware/caps with new2profile helper andtext assettransfer, buildisolation003PASS. Output/tmp/md-sim-original-boser-051, rootlogboser-browser-original051. Parent run copies captured helpersbeforeexecution. Fullworkspaceisolation001PASS beforelastasset/origin/cleanup edits; latestclippy003PASS. Re-runworkspace afterlatestchanges. Freeze080 and081each323files/library27edd2974d00ee7e52baecd63bda0d1866b055a6cad93c28d871b44d7d058c69/32186688B. These are sourcecheckpoints, NOT newQEMUbackend builds. Selectedbackend077 unchanged withownpinnedcore.081predateslastasset/origin/cleanup fixes; freeze082 neededafterchecks.
- Rootfilesystembrieflyreported0free. Cleanup-tests003/clippy002returned101 withemptylogs; no diagnosticbytes available. Afterverifiedduplicatecleanup, bothrepeatedtests004/clippy003PASS. Currentrootfree2.1GiB,/tmp17GiB. Nootheruserfilesremoved. Corecleanup008firstrejected/tmp sourcepaths with'Core source must be a declared local firmware run'; nojournal/mutation. Reclaimer nowaccepts onlyexactcanonical/tmp/md-sim-original-boser-NNN withterminalresult forsource evidenceCOPYdedup. Hashmatches stillrequired; noexpandedCASdelete eligibility. Strictpathnegative testsPASS2. Successfuljournal initially/tmp/md-sim-core-run-cleanup-008.json, copiedwork/simulator/qemu-s9/core-run-cleanup-008.json;52duplicates786792696Bfrom048/049fourCorechecks->KEEP030CoreCAS+original/tmp048/049sourcefiles. Browserprepcleanup0043503duplicates915469497B049prep->KEEPbrowsercaptured003. Rootspaceincreased2.1GiB; don'tattributeunmeasuredexternalchanges toours. Alluniquefirmware/evidence preserved.
- 14:44 UTC: Original051 FAILED468746ms exit1/notruncated. Two real Firefox profiles both started; second control port2829 worked. At the exact same origin, second profile displayed login, had no cookies and native auth.extend returnedAuthError (isolation-session-rejected at60875ms after firstauth). Whole run hit260s support-services deadline before final main result, so no isolation/fullbrowser/mining pass. First primary asset verification completed217731ms. Retainfailed/tmp/md-sim-original-boser-051. NoCoreclaimed.
- Two-profile suite now has an explicit300s completion allowance shared by support supervisor and parentwait; recorded inconsole.json alongwith browser_profiles=2. Existingminer360wall/330CPU, support600CPU, startup330wall, pool230window, ordinarydeviceclocks andallsemanticassertions unchanged. This is a new suite/resource identity, not a retroactivepass for050/051. Native WS now runs immediately after authenticating, whiletheprivatepoolisstillopen, beforeasset/uptime checks. It requestsrealboard/sensor/fan/pool counters using exact archivedfragments.
- Addedsupport/s9_browser_telemetry.rs: validates3unique boards6/7/8×63chips at100MHz, disabledtuner, Board25C/Chip65C explicitfixtures, positivefinite reportedvoltage,4unique fanchannels, orderedthermal thresholds, exactprivatepoolURL/user/activeALIVE, acceptedsolutions>0 andpool/summarycounteragreement. Reportednativehashrates retainproductionscale; neverusedasfunctionalhashoracle. Fullfault/calibration/nativeUIoperations/upload/SSH/TLS gatesremainopen.
- Extractedexactnative dashboardmodule020: www/boser/413.51a735efe893383edd72.js, SHA f244622121fc84e4e1e8ba6f90065516172b7c976d52fe0feb61764e83e3b51b/48403B, parentramdisk2bd49c70.../15491980B. Keptwork/simulator/native-s9/original-boser-dashboard-020/member.json. Mainapp1c318... module2879providesGraphQLfragments. Installedtshark independentlydecodedfailed050pcap c74ff1933216d3fbf77549483d7caf2c6ee7ef6b4b2dcbd1a3bbe55c7339e0d3. FirstactualBosminerInfoSample frame4518 savedfixtures/native-s9/boser-sample-050.json, SHAb1000d43f73c84c8b4113dd3f4b42c695be119c48d36ef067183b8eac298fa34; provenanceJSONbindspcap,tsharkSHA1b16ce...87d8, command, failedsource-status. Staticreal-dataregressionand8negative mutationsPASS; this fixture neverpasses source050 ormining. Rawdecoder/tmp/md-sim-boser-050-websocket-fields.tsv (84lines387931B), samplesjsonl retained.
- Original052 ACTIVE session56431 /tmp/md-sim-original-boser-052, samefirmware/backend077/pool003/cap1000/fixed4/clockepoch1792591709 +newtelemetry/two-profile suite300s. Buildtelemetry002PASS. Newfullworkspace telemetry001PASS, clippy002PASS, fmtPASS, guide013PASS393links. Counts stillneedsummary. Frozen083326files/library27edd2974d00ee7e52baecd63bda0d1866b055a6cad93c28d871b44d7d058c69/32186688B. Backend077 remainsunchangedandpinnedtoitsowncore; no083QEMUbuild. Freeze082323filespretelemetry. Nextinspect052actualJSON/fields, fix anyfailure, thenindependentaudit+twoCoretests onlyafterwholepassed. No final.
- Browserprepcleanup005 completed7006duplicateobjects1830938994Bfrom050/051prep stores->KEEPbrowsercaptured003CAS. Rootfree5.0GiB,/tmp16GiBat14:~35 (externalrootspacechangesunattributed). Alloriginals/uniqueoutputsretained. ExistingPIC16.md's unconnected-power statement appliesitsstandalonePIC developmentmachine; theASIC machine ALREADYconnectsPIC power-enable andGPIO reset toallthreeASICchains in s9-bm1387.patch. Do NOTduplicate thatimplementation basedonstaledocscope. Agentsstillquota-blocked; independentreviewgateopen.

- Original052 FAILED454122ms exit1/notruncated. Both real profiles started; second-profile auth rejection component passed. Primary WebSocket helper hit its20s deadline with no delivered events. Independent tshark stream287 shows server101 at381.389333s, delayed browser connection_init400.887521s, serverACK400.891381s (3.86ms), then browsercomplete/close411.54s withoutsubscribe. Native UI stream276 continued. This is browser event-loop delivery delay, not evidence of slow native acknowledgement. Retain all052 files and/tmp/md-sim-boser-052-{websockets,handshakes,stream287}.tsv.
- New helper runs its real WebSocket in a browser Worker, same origin/session/protocol/language/query;20s network bound remains,25s worker-result delivery bound and30s WebDriver script bound. Original page bytes unchanged. Original053 ACTIVE session13227, /tmp/md-sim-original-boser-053, same selected firmware/backends/1000H/s/fixed4/epoch1792591709/two-profile300s suite. Buildworker001 andclippyworker001PASS. Fullworkspace telemetry001 beforeworkerfix:90binaries312passed0failed0ignored. Freeze083 predatesworkerfix; repeat checks/freeze084 afterfix.
- Exact system asset static assessment savedwork/simulator/native-s9/original-boser-system-014/handler-assessment-001.json. Original frontend handlers for upload,password,network onlynotifyTODO;noassetrewrite. This does not establish absence of LuCI/CLI update routes or pass update/recovery/dashboard gates. Continue testing real supported paths. Agents remain quota-blocked untilOct25 08:48; independentreviewgateopen. Root351GiBfree and/tmp13GiB atthischeck; large rootfree increase external/unattributed, noarchivecleanupperformedhere.

- Original053 FAILED466120ms exit1/notruncated. Both profiles cleanupempty/noOOM. Native UI WSstream409 worked; diagnosticworkerstream430 handshook417.895915s thenping/pongcontinued butno connection_init beforeparent25sworker-deliverydeadline. Primarygroupthrottled124119202us,totalCPU228018694us;secondaryCPU149905035us/throttled57432861us. No semantic/CPUbudgetchange. Worker now starts on native login page withoutnetwork; its actualreadytime is retained and readinessrequired beforebounded20snetwork/25sdelivery test. Original054 ACTIVE session81105 /tmp/md-sim-original-boser-054. Warmbuild001/clippy001/JSsyntax001PASS. Same selectedoriginalfirmware/backends/caps/clock/suitebounds. NoCoreclaimfor053/054yet.
- Added GatewaySpec.browser_policy enum preserve(default) orupgrade-insecure-requests. Opt-in emitsrealContent-Security-Policyresponseheader; nooriginalbodyedit,noTLScheckdisable. Endpointmanifestexplicitlyrecordspolicyandscope. Backwarddeserialize/defaultandunknownpolicyrejectiontestsPASS. Newseparate-client protocolfixture001PASS41685ms exit0/notruncated; strictfrozenreporterPASS. Sameexactrun230363be30bb7d2002973257b0f43c133a6ecee627a0ffe366bab5ad6b46cf74/9804B forSSH,mTLS,artifacttransport,uploads,streaming,WebSocketchallenge/echo/close; realHTTPand101headersbothcontainoneexactupgradepolicy. Unauthenticated/wrongHost/wrongOrigin/wrongCIDRrejectionstillpass. LocalisolatedVMwithdistinctclient/workernamespaces;NOTexternalphysicalhost/nativebrowser/originalBoser/mininggate. Full/tmp/md-sim-gateway-policy-001; compactwork/simulator/network-vm/gateway-policy-001; frozenbinaries/scenario/identitiesgateway-policy-driver-001. Runtimeandreplaycommandsincheckslogs.
- Latestworkspacegateway-browser-policy001PASS90binaries313tests0fail0ignored; clippy001PASS. Sourcecheckpoint084stillneededafterREADME. Browser054captureds9helperbeforegatewayedit;componentgateway001capturednewreporterandpolicy. Existingnative024keepsdefaultpreserve;itsoldresultcannotpassoriginalBoser.
- PublicBM1397search foundsamecommunityregistermapwithoutordered-clock-enablebitsemantics; existingMujina contractalreadycoversBM1362/66/68/70andexplicitlyexcludesBM1397jobformat. Newreferencecandidatehttps://git.warppool.org/dvb-projekt/dvb-WarpAxe at9cbc497187(2026-10-20)hasRustBM1397driver/simulatorclaims,butnotdownloaded/pinned/reviewed;doNOTinferphysicalmodelvalidityorchangefrozenmatrixfromwebclaims. NoBM1397modelchangeornewpass.

- Original054 FAILED461613ms exit1/notruncated. Warmworkerready in686ms beforelogin, so053 startupdiagnosis isnotconfirmed. WorkeropenedWebSocketbut noeventdeliverybefore20snetworkdeadline. PrimaryCPU250925300us/throttled140415051us;secondaryCPU153335581us/throttled61283696us; bothcgroupsempty/noOOM. Warmingalone didnotresolvefailure.
- NewexplicitS9browsersupport resourcecontract4VMCPUs/2CPUsperbrowserleaf; otherbrowserdriversretain1CPU. CPUreadbacknowretainedincleanup. 2GiBVM,768MiBperleaf,256tasks,300scompletion,20snetwork,25sdelivery,330minerCPU/360wall,600supportCPU unchanged. NoASICcap/filter/guestclockchanges. Original055FAILEDduringpoolimagepreparationbeforefirmware. Baseseparatepreparationexit0/220ms; enclosingpayloadtoolfailedwithoutstderr/statusdetail. ConcurrentworkspaceCPU001linkerexited101(signal7buserrorreportedbycollect2). DoNOTinferOOM/diskfullwithoutdiagnostic;dfshowed/tmp12GiBfree/root351GiB. Failurefilesretained.
- Movedown2.7GiBdevbuildcachefrom/tmp/md-sim-checks-069 towork/simulator/check-target-085; noarchiveoruniqueresultremoved. NewCARGO_TARGET_DIR=work/simulator/check-target-085. Addedpoolpreparationexitstatus/signal/elapsed/stdout+stderrhashrecord,buildcpu002PASS. Original056ACTIVEsession48191 samecommandas055/newoutput056. WorkspaceCPU002ACTIVEsession14879 onnewcache.
- Browserprepcleanup006PASS10509duplicates2746408491Bfrom052/053/054->KEEPbrowsercaptured003CAS,exactcopyrestorejournalwork/simulator/qemu-s9/browser-preparation-cleanup-006.json. Currentroot348GiBfree/tmp15GiB. Frozen084326sources/library27edd297...d058c69/32186688B includeswarmworker+gatewaypolicy, predates4CPU/preparationstatus. Guide014PASS394links,fmtPASS. Freeze085neededafternewchecks.
- Foundsupportednativeupdatepath: actual054GraphQLresponseframe4135advertises/cgibinLuCI systemLinks, including/cgi-bin/luci/admin/system/flashops; source054stillfailed. Exactoriginaluhttpdconfig021SHA302128daf55df98b268cc7296326a8fd33786747da9b3ca58762730aa675b077/3784B listens8000. ExactLuCIsystemcontroller022SHA60f6532f0362e08c4d445d47d43f5b318dc272ec3dbd57f04b090db9ac4a1f00/11532Bhasrealbackup/restore/sysupgradevalidationhandlers. BothisolatedextractionsPASS; parentramdisk2bd49c70...7154b3. ui-link-observation.jsonpinsactualpcap+decoder+frame; noexecuteduploadorupdatepass. EarlierBoserTODOhandlers doNOTexcludeLuCIroutes. Next testnativeadvertisedlinks/uploadvalidation/backup beforefullupdate/recovery.
- 15:34 UTC: Original056 failed before Linux after60ms, exit1/no truncation. Four-CPU support QEMU could not mmap its initramfs under3GiB RLIMIT_AS. Preparation succeeded52671ms. Added explicit4GiB support address allowance, then lower supervisor/miner to original3GiB after support spawn. Miner330CPU/360wall and all guest RAM/hash/clock limits stay unchanged. Actual resource-limit regression checks soft/hard RLIMIT_CPU and RLIMIT_AS. Original057 active /tmp/md-sim-original-boser-057, session24319; four VM CPUs/two per browser. Both profiles now start in49792ms; firmware/native suite still pending.
- Moved-cache test CPU002 failed3 queue tests because cached CARGO_BIN_EXE paths pointed to old /tmp location. Cleaned only moved generated build cache and rebuilt in work/simulator/check-target-085. Fresh address-workspace-tests001 PASS90 binaries313 tests0 failed0 ignored. Strict address-clippy001/fmt001 PASS. Release address-build001 PASS. Source085 frozen326files/library868d0d1845e1d31f0abc55c9b5d2d34ded6e6b2a1d6aa20604f8f59d299b93f6/32186688B; predates LuCI test extension. No085 backend built.
- Implemented opt-in dashboard-luci extension to real native HTTP suite. Adds separate captured contract, explicit30request bound (basic remains20), method/CSRF rejection, backup download decoded without extraction, and invalid multipart firmware upload rejection. No flash/restore/step2 requested. Original dispatcher024 extraction verifies32hex CSRF token and optional separate LuCI login. Unit tests7PASS; real execution pending. Original057 uses already captured prior helper and is unaffected. Exact physical target/full matrix/independent-review gates remain open; agents still quota blocked.
- 15:42 UTC: Original057 PASS357097ms exit0/no truncation. Two-profile native browser PASS158949ms after49792ms startup. Same-origin second profile has empty cookies/native AuthError while first remains authenticated. Full native telemetry validates3boards6/7/8×63chips100MHz,25/65C explicit fixtures,4fans,privatepoolALIVE; WS samples1792591975/1792591976. Four support CPUs/two per leaf resolve observed throttling failure here. Both groups cleaned. Independent audit42154hashes54serial replies53submissions=23accepted30LowDifficulty,2freshjobs/clean/difficulty,21additional candidates. Core0/1 retry001 PASSheight0->1/restart:08e903da01d0df24c125eef684ed21d98e16313abc97350c97fa5728f96d4a50 and018ac0aa00272ca09d93239e1e4ca03a0cf24154c2ecb45efba331af88d0117b. Initial CorecommandsFAILEDreplay becauseauditwasoutside sourcedirectory; no nodeexecution. Copiedidenticalaudit37d58706b115ee3d227a7491e55967461a70f72714e6827b6b62a9fe28b99cbf into/tmp057/regtest-audit-001.json; newoutputswork/bitcoin-core/original-s9-057-candidate-{0,1}-retry-001. Compact057evidence/sumsinwork/qemu-s9/original-browser-057. README/NATIVE-BOSERupdated. Physical/fullmatrix/SSH/TLS/originalBoserupdategatesstillopen.
- Original058 LuCI probe FAILEDtokenformat: nativeflashops actualHTTP200 viaBoser session, all3hiddenCSRFtokensliteral0000. Standarddispatchercreates32hex, butactualBoser integration differs. Exactoriginaladmin025+CGI026 extractionsPASS; nopatches. Correctedparseraccepts pinnedliteral0000 orstandard32hex, rejectsother4charvalues; wrong-token403assertionunchanged. No random-token/security-strengthclaim. Original059ACTIVE session21681 withdashboard-luci,newreleasebuild003. Original058 retainedfailed; noCore/auditpass. LatestfullworkspaceLuCI001PASS90binaries315tests, strictclippy002PASS beforetokenfix. Recheckneeded. Actualupload/backupstillpending.
- 15:54 UTC: Original059 PASS237322ms exit0/no truncation. Native LuCI22requests: advertisedflashops viaauthenticatedBoser (no separateLuCIlogin), backup7626B compressedSHA0c1bed17fd7bfcd5633e36fd499cad377507d2abec4611f2dbef18a1c80eeb7b,45568B decoded/36entries/activeprivateIP; wrongtoken403,GETbackup405,invalidmultipartimage rejected. Nativeuptimecontinues. Audit28165hashes29ASICreplies29submissions=21accepted8LowDifficulty,2freshjobs. CorebothPASSheight0->1/restart:07307b070370806f54ea14acd6fb77b7c0ccb3bc81563325428313823b0fa9b4 and01914a3c03b8e80cda5925be88a637dd65f2832674714c39e65bd7d0a26a0a98. Compactwork/qemu-s9/original-luci-059; fullraw/tmp059. Failed058 retained237774ms exit1/notruncated.
- Implemented opt-in dashboard-update-validation for exact signed22.02.4SDupdate braiins-feed-25bba3b4a505ccbc0e54,19179888B/SHA469743ebcd5faf3445e4f4209ad8af58cc7adeff37c7df28af3c53a5b255b56e. Actualarchivehashverified. NANDvariant2617e1f...SHAfa8d4da0e07eef884a24100061589854b2a2abf7bea0c12970ce02585d54f70a isdistinct/notselected. EmbeddedSDfirmware.tar.gz37ade53...isNANDtransitioninstaller, not thissignedSDsysupgrade. UsedcatalogSQLiteexactpath; rg defaultignoredarchivepaths gaveincompletefilelist. DoNOTassume missing based on that search. Webfetchexactfeedgavecachemiss butlocalarchivehasboth. Originalplatform027+upgradeview028 isolatedextractionsPASS. Platformusesoriginalsignature/modevalidation andSDwritepath. No hostinstallerexecution.
- Newuploadsuite capturesexactarchiveandcontract,allows60s onlyforlargeupload, retainsfullpacketpayloads with24MiBcapturebound (normal1MiB/browser16MiB unchanged). Outer64MiBtrace/file caps unchanged. Native returnedSHA256 andconfirmationcontrols required; sendscancel only, requiresredirectandunchangedsession/uptime. No step2flash/request. Runnerwaitsforactualcompletedvalidation/cancellation beforestoppingminer;230spool and330CPU/360wall unchanged. Newnegativecompletionregressionadded. Suiteenumkeepspoolprepare6explicitparameters. Build002PASS. Actual061ACTIVE session33119 /tmp/md-sim-original-boser-061,rootlogboser-update-original-061.log. No validuploadpassyet.
- 15:50–15:53UTC temporary/tmpuserquota: newexecsandboxfailed synthetic/tmp/.git mounttargetEDQUOT; apply_patchfailedwrite. Existingemergencybash70212remainedusable; usedsameauthorizedworkspace tocontinue. Fullworkspaceupdate001FAILEDgatewaytemporarycertificateEDQUOT;060FAILEDbeforefirmwarewithEDQUOT. Diskgloballyhad9.4GiBtmp/346GiBrootfree. Hash-verifiedduplicateprepcleanup007removed7006files1830938994Bfromfinished056/057CAS->KEEPbrowsercaptured003. Uniqueinputs/results/originalsretained. Newexecworksagain; tmp12GiBfree. Freshworkspaceupdate002ACTIVEsession44533; clippy003/fmt/guide016runningafterlatestREADME/help/testchanges. Source085predatesLuCI/newupdate;freeze086neededafterchecks. No independentagentsavailablequotaunchanged.
- Original061 FAILED238297ms exit1/no truncation. SignedSDuploadreceivedHTTP100Continue only; curl60sdeadline expired with0responsebody. Full20MiBpcapretained. Installedtshark TCPstream239 proves19180318Bmultipart bodyfinished97.699578s andminerACK19180631at97.723637s, starting94.817603s; wholeupload~2.9s, thenno finalresponseuntilclientFIN154.815140s. Thusnot slowtransfer; no deadlineincrease. TSVevidencework/qemu-s9/update-061-{http-requests,http-continue,upload-stream}.tsv. Firsttsharkqueryfailedinvalidfieldhttp.expect; correctedfieldsused. NewlargeuploadclientdisablesExpect header to matchbrowserFormDatabehavior, keeps60sbound, addsactualcurlupload/download/time metrics toallrequests. 100continuefailure remainsunresolvedoriginalserviceedge; notpassed. Build003active before062repeat. Frozen086328sources/library868d0d18...b93f6predates thischange. Updateworkspace002PASS90binaries317tests/strictclippy003/fmt/guide016PASS beforeExpect/metricschange.
- Original062 FAILED238155ms exit1/no truncation. Expect-disabledrequest also uploadedcomplete19180318B multipart body,0responsebytes,60002mscurl time. This doesNOTconfirm Expect asrootcause. Keep both061/062 failures;60sdeadlineunchanged. Build004adds read-onlyoriginalguestdiagnosticafterconsole:actualuptime,uploadedfilesize/SHA256,ps,andnativeworkerstat/wchan/syscall. Itdoesnotrunanother validator orchangeuploadfiles. Original063ACTIVE session43652 (commandlogboser-update-original-063.log), NEWroot-backedoutput work/simulator/qemu-s9/sd2022-private-pool-063 toreduce/tmpquota pressure. Samepinnedarchive/backend/pool/hashcap/clock withExpectdisabled. GoalinspectrealLua/uhttpd/boser/validatorwaitstate whileuploadactive beforefailure. No update/mining/Corepassforfailed061/062. Last317testsPASSupdate-workspace003/strictclippy004 beforediagnosticaddition; clippy005/fmt002runningafter. Emergencybash70212stillalive; newexecworksaftercleanup007. Fulltaskremainsincomplete;continue063diagnosis,fix,rerun,thenactualupdate/recovery/SSH/TLS/targets. NO FINAL.
- 16:14 UTC: Original063 FAILED238114ms exit1/no truncation. Root-backed work/simulator/qemu-s9/sd2022-private-pool-063 preserves full traces. Read-only native diagnostic found exact uploaded image SHA469743ebcd5faf3445e4f4209ad8af58cc7adeff37c7df28af3c53a5b255b56e/19179888B at /tmp/firmware.img. Original sysupgrade -T was in firmware_check_signature: cat | head -c -167 | sha256sum. head was runnable, SHA waited on pipe; no fabricated confirmation. Full HTTP upload had completed. Single-thread QEMU handles both guest CPUs; next test selects existing multi-thread TCG, retains firmware/network timeouts and records execution mode. Diagnostic head001 failed because init=/bin/sh had no /dev/zero; head002 adds original devtmpfs mount. Neither is update acceptance. Source087 has328files/library868d0d1845e1d31f0abc55c9b5d2d34ded6e6b2a1d6aa20604f8f59d299b93f6/32186688B. Latest workspace update003 passed90binaries317tests, Clippy005/fmt002 passed after diagnostic addition. Agents remain externally quota-blocked; independent review gate open. No exact-target completion claim.
- Native head measurement003 PASS11192ms. Original BusyBox302e914fd6189817a378e5393918dcda55ec34a779a5aa67586b68b766594aca and libc5c34fa1916cae5f7a706538593e0f5fc462993776f2a52e2fd6fc853f3055720 run in archived kernel/rootfs on installed QEMU, single CPU/init shell. Native negative/positive head both match:1MiB-minus167 SHA8953b2399f2dfaa9799bacd4e4eb195c16bbb6a495e8f2ae69f6be8e53ae3a83 (~0.5s);4MiB-minus167 SHAe9bdf22226f4f5b86790dfe2b30c38dabd75fe1104f0a4d6ef1c108d94496a8b (~2s). This is a utility timing diagnostic, not validation or firmware acceptance. 001 failed missing /dev/zero;002 failed unsupported devtmpfs;003 explicitly mounted private tmpfs and created guest zero/null nodes. All original executable bytes preserved.
- Source088 frozen328files/library868d0d1845e1d31f0abc55c9b5d2d34ded6e6b2a1d6aa20604f8f59d299b93f6/32186688B. Optional tcg-threads=single|multi records exact selection in process argv and clock manifest; default unchanged. Invalid value and multi+icount commands fail before output/guest creation. StrictClippy006/fmt003/workspace004PASS90binaries317tests0fail0ignored. Full original064 multi-thread test ACTIVE55214 but no guest/device progress after virtual55.87s. Native HTTP fails at early authentication request; validation not reached. Thread-state observer in work/qemu-s9/observe-child-001.rs will inspect only owned descendants on a new diagnostic run; no environment or command-line collection. Do not claim multi-thread mode passes.
- Original064 FAILED311966ms, exit1/no truncation. QEMU consumed~329.06CPU-user seconds before the bound; last device time55.873922068s. Therefore the stalled multi-thread mode is not merely sleeping. Original065 FAILED177938ms under180wall/170CPU; stalled earlier during serial input. First observer matched the wrong executable comm (qemu-system-arm vs actual qemu), so retained only owned descendant PIDs, not thread samples. Revised temporary Rust observer002 matches actual run-owned qemu, records bounded thread stat/wchan/syscall, and at100s execs installed GDB as the owning ancestor for a stack capture. It then terminates only the launched runner. Run066 is a deliberately interrupted diagnostic, never an acceptance attempt. All full failures remain retained.
- 16:33 UTC: 066 deliberately interrupted at100s by owned observer002. Thread samples show both TCG CPUs progressed in this attempt, unlike064/065; no backend fault was identified. Installed GDB attach was denied EPERM despite ancestor exec; no approval request or repeat of the unchanged kernel restriction. No acceptance result for066. 067 single-thread/cap100 also FAILED: actual full upload19180318B, native request timed out60s, head still running in signature pipeline. Reduced mining work did not remove the validation delay. Next068 selects the explicit one-CPU diagnostic with original1000H/s, unchanged network/validation deadlines; it changes execution identity and cannot establish the two-CPU board gate. All requested exact-target gates remain incomplete.
- 068 one-CPU diagnostic also FAILED the60s client upload bound. It was slower on earlier native operations (backup6755ms, small invalid upload4040ms). Archived kernel has no /proc/cgroups; taskset applet absent. Affinity workaround was not implemented. The60s cap belongs to our newly added curl transaction, not a specification requirement or measured native deadline. Keep connect2s, ordinary HTTP10s, original firmware/uhttpd timing, pool230s, CPU330s and wall360s. Increase only the complete19MiB signed-upload client operation to120s and bind this budget in the captured update contract. Keep all native format/signature/SHA/cancel assertions. Remove the temporary in-guest process diagnostic from the normal suite; original063/067 retain it. Next069 tests whether the real native validator completes under this measured, bounded client allowance. This is not an installed-update or exact-target pass.
- 069 FAILED with the old60s client: rebuilt s9_pool_link rather than the actual s9_pool_guest. Captured helper identityfb8827aa9d0fa3a42b78ee4804ddb54ed2f41191d0010b37a201badcce5a177b proves the stale client. This run does NOT test120s. Fix build command to include s9_pool_guest. Bump signed-update contract to/2 so old helpers reject the new policy instead of accepting ignored new fields. This is a build/orchestration error; retain the failed evidence. Source089 predates schema/2 and new update-bundle inspector. New isolated s9_update_bundle inspects all seven original signed-tar members, hashes executable components, exports COMMAND/CONTROL as nonexecuted reference text, and compares objects to the supplied baseline bundle manifest. Initial build001 failed Vec-vs-array/unused-mut and was corrected before execution. Source090/build008 in progress; next full070 uses actual new helper.
- 16:49 UTC: 070 FAILED238785ms exit1, trace_truncated=true. Correct new pool client1ee88fe8de373e32981f0cc8803a6689e32ff35600fc24a12272cad2be9c09ed/1848016B ran. Its signed upload was limited to89.005272s by remaining230s suite time, not the full120s per-operation bound. Full19180318B multipart body uploaded; no response. Aggregate stdout38949951B/backend28158913B exceeded64MiB and truncated packet JSON; retained raw logs/results, no packet or acceptance pass. Correct resource policy: selected signed-validation suite pool300s, total trace128MiB, per-file64MiB, CLIwall>=330s (usual360), CPU330s unchanged. General pool input maximum300s; boundaries19/301/u64MAX rejected. No firmware or connection/ordinary-request timing changes. Schema/2 contract pins these bounds. Full release all-target build002 and workspace006/Clippy009 running before next071.
- Removed experimental tcg-threads CLI from active code. Failed064/065 and interrupted066/source088–090 preserve experiment and identities. No multi-thread miner support claim. Source091 captures cleaned code plus signed-bundle inspector. Signed-sd-update-002 isolated inspection PASS; exact SPL/UBoot/FPGA/FIT hashes match baseline, COMMAND82b03ae21d63222d070c48cee66dcf29c0855b98e4b80bf5cd45ec98eded0f7c/18498B and CONTROL60eace2dededf0595e8703987a99a1dcea18349092af5d1f8bc5770b8ebc27ef/13B exported as reference only. Native COMMAND check_image/pre_upgrade run only inside firmware later. README adds actual tested inspection command. Signature validation, installation, restore/recovery, original-Boser remote access and every exact-target matrix gate remain open. No independent agents available due unchanged quota.
- 071 FAILED before firmware execution: worker Limits validation still capped total trace at64MiB. Prepared inputs retained; no result.json or guest execution claim. Correct hard ceiling to128MiB, with default trace1MiB unchanged and per-run reservation still3×trace plus metadata. New boundary test accepts128MiB and rejects zero/128MiB+1/u64MAX. Existing shared-stdout/stderr truncation and mount-safety tests pass.
- Review found SerialGuest did not drain its output while waiting for a support-browser or update result. A guest log burst could block QEMU on the full pipe. Added bounded poll/read forwarding for these waits, stops reader before next console script, rejects unexpected EOF, and releases reader during unwind. Actual OS backpressure test retains1.16MiB exactly; EOF test rejects closure. Initial test failed because UnixStream write uses socket-send calls denied by host; changed both endpoints to File/POSIX read-write, same transport used by QEMU. serial-drain-tests-002 PASS2; trace-budget-tests-001 PASS3. Generic supervisor change needs full original update and browser reruns. Source092/release-all003/workspace007 now building. Do not claim previous057 browser identity covers the new supervisor. Next072 reruns signed validation with full120s/300s/128MiB policy and serial draining, then audit/Core only after whole run passes.
- 16:58 UTC: Release-all003, strictClippy010, fmt006 and workspace007 PASS (91binaries320tests, zero failed/ignored). Source092 freezes329files/librarya0d81aa0acc5da79c7f67bc3001dcaeab0f612022fd6c148b55eb0b16a9bb6fe/32174092B. Backend remains077 with its own pinned core. New original072 ACTUALRUN launched with default two CPUs, single-thread TCG,1000H/s,120s signed upload client allowance,300s pool,128MiB total trace, original firmware timeouts, and serial draining. Wait for full result before mining/Core audit. Active session is recorded by the tool call; root log boser-update-original-072.log. Browser rerun still required after serial supervisor change.
- Original072 FAILED307965ms, exit1/no truncation. The correct d79b7001d1814178c7e77cad470b14d7da30659a46646aafa9e606bd52fb17a1 client fully uploaded19180318B multipart. Native HTTP502 arrived after72.229390s with60B body: `Bad Gateway / The process did not produce any response`. This is a real native CGI failure under the original uhttpd script_timeout60, not the old client timeout. No mining/Core pass claimed. All evidence remains in work/simulator/qemu-s9/sd2022-private-pool-072.
- New maintenance suite/3 uses the exact archived frontend's Bosminer stop/start GraphQL operations. It waits for two actual accepted-job observations, requests native stop, requires TCP4028 ConnectionRefused with advancing native HTTP uptime, then uploads/validates/cancels and requests native start. It requires two accepted jobs on a new connection afterward. Progress observations only synchronize the test; independent header/hardware audit and Core acceptance remain required. Original native firmware/CGI timeouts remain unchanged. Restart is attempted after validation failure and both failures are retained. Focused maintenance/helper tests001 PASS10.
- Review found pool reconnects reused input.seed for every connection. New frozen pool-input schema/4 derives a domain-separated SHA256 seed per connection in both local and separate-VM transports. Schemas2/3 retain their original seed semantics for reproducibility. Reports retain the actual session seed. Tests prove16 distinct connection seeds, deterministic frozen replay, bounds and legacy behavior. No test run uses this change yet. Full release build and strict Clippy are running before original073. Agents remain unavailable under the unchanged quota; independent review gate remains open.
- Full workspace maintenance001 caught a stale expected schema3 assertion after the schema4 change. Corrected that assertion; no behavior assertion was removed. Also require the native update completion marker to carry the current maintenance contract and both actual stopped/restarted port observations. Source094 captures330files/librarya0d81aa0acc5da79c7f67bc3001dcaeab0f612022fd6c148b55eb0b16a9bb6fe/32174092B. Source093 preserves the earlier state. Release-all002, strictClippy002 and fmt002 PASS; workspace002 finishing. Agent status query confirms all three still quota-blocked untilOct25,2026 08:48AM.
- Original073 FAILED360043ms with outer wall timeout/no truncation. The real native stop mutation returned VoidResult after two accepted jobs. The native safety monitor then wrote100 to0x42800018. Backend077 rejected that unsupported write and stopped QEMU, so the next uptime request failed. No upload validation, restart, mining or Core pass for073. Full raw evidence remains retained. The guard exposed a real unimplemented/uncertain interface; no return value was fabricated.
- Exact original monitor extracted in isolated original-bosminer-monitor-031: usr/sbin/bosminer_monitor.lua SHA5dd1adc64ed54033c2b9ed6c1998de31c744d133fbb4bfc77f11bef30b3f2a1f/11880B, parentramdisk2bd49c70.../15491980B. Lines196/203/213 use N_FANS=2, base0x42800010 and loop1..2, so they write offsets20/24. The declared two-PWM model exposes16/20. This does not prove a physical firmware bug or the2022 FPGA map. Extraction030 used wrong usr/bin path and failed; retain that failure. find-native-fan-001 read-only original-shell inspection also passed.
- Implementing optional explicit adapt-monitor-fan-source=EXTRACTED_MEMBER. Rust prepares a one-line source patch (loop0..N_FANS-1), exact adapted bytes, hashes/provenance and bounded guest application script. Guest must verify original or already adapted identity and the final changed hash before continuing. The native monitor and its protective writes remain active. No model access is silently accepted. This adds a patched-native-script reference path, not original-script/physical-board conformance. Archive originals remain unchanged. All affected checks and an actual run remain required.
- Independent audit now has an explicit regtest-session selector so a restarted native miner's connection can receive its own full hardware/header/Core evidence. Selection rejects missing/duplicate IDs and does not select a passing fallback. It binds the actual seed/time/anchor to the frozen pool input. Core replay uses that same session ID. Reader bounds now match the128MiB runtime trace ceiling plus hexadecimal JSON expansion. These changes are under test, not yet accepted results.
- Source095 freezes331files/librarya0d81aa0acc5da79c7f67bc3001dcaeab0f612022fd6c148b55eb0b16a9bb6fe/32174092B. session-audit-monitor release-all002, Clippy002, fmt001 and full workspace001 PASS91binaries330tests/zero failed/ignored. Focused tests001 also pass. New explicit-session verifier replay of raw original059 session0 PASS:28165computed hashes/29serial replies/29submissions and the same two prior Core candidates07307b... and01914a... . Report /tmp/md-sim-original-boser-059/regtest-audit-session-002.json. This is a replay regression, not a new native run or new Core acceptance.
- Adapted074 ACTIVE session47126 at work/simulator/qemu-s9/sd2022-private-pool-074, log boser-update-adapted074.log. Same077/003 backends,1000H/s/fixed4,300s private pool,120s signed-upload allowance,128MiBtrace,360wall/330CPU and original CGI timing. Explicit monitor flag uses extracted031 source. Prepared adapted Lua SHA720c760f4948e5c23a5b4146496e80f063ca8a1ef6eb40c6b97e641b12900da2/11884B, patchd9baa9bcc126c7daa0464b74e86559ad65dcef51335503334e58803f5d97f77a/214B. Guest readback still pending. Do not call074 original-script execution. Next inspect actual stop/validation/start results; audit both mining sessions and Core only if the entire run passes. Full dashboard/browser rerun after supervisor change still open.
- Adapted074 reached real native stop and advancing uptime. Guest verified adapted monitor SHA720c760f...00da2 and executed protective writes at16/20 successfully. Then PIC0x56's timer expired at117840746728ns, returnedstate0x8 and stopped QEMU. Its final heartbeat was107840717966ns. The original miner had already sent explicit ENABLE_VOLTAGE=0 at108623211360ns; power pin recordedstate0x6d02. Current PIC model arms its10s deadline even when disabled and treats recovery as unspecified. No deadline extension, fabricated heartbeat or recovery has been implemented. Run still waits for outer terminal evidence. Need establish exact app behavior from archived PIC words/source before changing reset/recovery. Driver power.rs explicitly disables voltage in its shutdown handler, but does not establish post-timeout I2C state. Full PIC CPU baseline/loader and standalone emulator remain missing.
- Public search found only already known driver contracts and older FPGA sources, not exact2022 fan HDL or PIC application source. Primary opcode reference: Microchip PIC16(L)F1704/8 DS40001715D, https://ww1.microchip.com/downloads/en/DeviceDoc/40001715D.pdf . Gputils supports PIC14E/PIC16F1704, but gpsim/gpdasm/gpasm/rizin/radare2 are absent from installed tools and package cache. No new CPU emulator is authorized or planned. A bounded static instruction analysis can help establish the exact archived app's watchdog path; it is not instruction execution. Public binary acquisition remains constrained by host networking. No unsupported state is counted as passing.

- Adapted074 terminal result FAILED: timed-out, null exit,360040ms, no trace truncation. Preserve all evidence. Its native stop observation passed before the old PIC model stopped QEMU. No upload/restart/mining/Core pass is claimed.
- New pic16_static_listing is a bounded Rust linear decoder, not a CPU emulator. Exact text ec84da07ef98665c6258cc531dc2d74aa8be7018925c041325ca233aec52b46a/19200B and word-stream d138aefed416e18a7c5f2a374ef99148c745f7d576b3821e615225fffad1d35b/6400B. Static001 decodes3200words; zero unknown opcodes does not prove code reachability. Tool/source/input/listing hashes retained. Literal opcode tests pass. Independent disassembler and separate author review remain unavailable.
- Manual bank/page-aware analysis establishes command15 at0478..0481 controls active-low PORTA.RA2; command16 dispatches to0abc..0ac1 and clears the elapsed counter/divider without changing power. Mainloop0f0c..0f0e skips those counters while power is disabled. Timeout0f35..0f39 disables power and clears the elapsed counter, then0f3a CLRWDT continues the app. RESET0f66 belongs to the separate command07 flag. Full reasoning and limitations: simulator/firmware/pic16/heartbeat-analysis.md. These are static control-path findings, not PIC execution or calibrated timing.
- Fixed PIC contract to pause remaining heartbeat time while disabled, preserve it across enable/disable, reset it only on actual heartbeat, and cut power at expiry while retaining application access. Repeated ENABLE does not feed heartbeat; heartbeat alone never restores power. Original10s timing estimate retained; Timer0 phase/oscillator/config/loader remain unresolved. Removed obsolete fatal-expiry state/status8. Three new regressions fail on old model, all8PIC tests pass after fix. C ABI tests pass ASan/UBSan with existing LSan-disabled restriction. Compiled qtest now requires exact power timestamps, three raise/lower pairs, disabled wait20s, actual scheduled cutoff and version access after expiry. It is not yet a QEMU pass.
- Source096 freezes333files/library8724232fc955236382ed240ee8abcbfe2614d7ac39c36c0aa796b42f93761c3f/32172478B. Release-all001, strictClippy001, fmt001, guide402links and workspace001 PASS92binaries333tests/zero failed/ignored. Active backend078UBSan build session35397 writes work/simulator/qemu-s9/backend-078-ubsan; source031 overlay reused read-only. Old077 compiled regression active session9559, outputpic16-heartbeat-before-077. No timing-sensitive firmware run active. Next require new078 PIC and BM1387 power qtests, then adapted075 signed validation with exact074 monitor adaptation. Only after full firmware run passes: audit both mining sessions and submit each session's fresh block candidates to Core. Full browser after serial-drain change, install/restore/recovery, actual-Boser remote, original-board and all exact matrix gates remain open. Do not final.
- Backend078 build and eleven smoke tests PASS. QEMU a3f5fccbb9cfd362e552a7e2c41a7086604e42cf40093a18458c5ae79e88a7fc/19883704B, pinned to096core. Old077 PIC qtest FAIL as expected at pic16-heartbeat-before-077/assessment.json. New078 PIC and connected BM1387 power/reset/temperature qtests PASS in pic16-heartbeat-qtest-078 and bm1387-heartbeat-qtest-078. Exact579I2C responses/seven power timestamps/three raise-lower pairs checked. No calibrated PIC execution claim.
- Adapted075 PASS exited0/308386ms/no truncation at work/simulator/qemu-s9/sd2022-private-pool-075. Native stop refuses4028, HTTP uptime advances, exact signed19180318B multipart validates with HTTP200 in39570ms, native cancel302 returns flash page, native start listens after14675ms, native logout/rejection pass. Explicit monitor patch hash720c760f...00da2 applies; backend078 PIC does not expire while disabled. No installation requested. Audits regtest-session-0.json and regtest-session-3.json PASS, distinct schema4seeds. Shared10290hashes/10serialreplies; session0five submissions/3accepted/1stale/1lowdiff, session3five/3accepted/2lowdiff. Each two fresh jobs, clean transition and difficulty change. Four Core directories adapted-s9-075-session-{0,3}-candidate-{0,1} PASS height0→1 and restart persistence. Hashes0aad038a03ec5273c9f19f451610637d3e3eef55e19f8b45b1dbbf7b6c1947fe,065d4ca900b3d785f468a0cbed1e035fdc53ca0fc22cca6704cd03651faee39a,04d67c1e037d8dcd1f33000870ea4c87fb5b0abb3e4e15c579fa018214647270,0423de0d02d436eab56286b122728e202856eac322c59be8333658b7d0a64b84. Explicit-session Core replay of old059 also PASS in original-s9-059-explicit-session-regression-001; replay is not fresh execution.
- Original-script browser076 FAILED exited1/283072ms/no truncation at work/simulator/qemu-s9/sd2022-browser-076. First Firefox profile actual DOM login/assets/auth/WS and second empty-cookie login page passed; second auth.extend JSON parse failed. No OOM/pidmax. Packet stream266 proves native server408 emptybody at251.874259s, FIN251.880131s, before first client POST252.104630s. Reports browser076-http-responses.tsv and browser076-stream266.tsv retain exact frames. Not an auth pass. New browser code retains raw HTTP status/body/parse error and permits one empty408+Connectionclose retry only for the unauthenticated session check, sharing30s deadline, then still requires AuthError. Unit mutation test passes. Actual repeat remains required.
- Native signed-install code added, not yet executed: dashboard-update-install selects exact pinned image, native stop/validation and step2 keep=1 commit, then observes kernel restart text+serial EOF+actualQEMUexit0 under-no-reboot. No kill counts as restart. GuestRestart hashes retained serial bytes. Storage schema3 records restart separately and explicitly does not claim an issued sync. Old schemas remain supported. Exportgzip bound24MiB for actual rewritten19MiB firmware, raw32MiB/virtual128MiB unchanged. Failed runs remain failed. Later boot must inspect installed files/native services/preserved settings and mine fresh blocks. New CLI usage includes both update actions.
- Targeted native-install-tests003 PASS after fixing explicit test closure lifetime and WebDriver timeout u32 compile errors in001/002. Native-install strictClippy001, fmt001 and release-all001 PASS. Full workspace001 active session80025. No firmware active. Next freeze097 after docs, then actual signed install using provisioned cycle006SD state and initial console, explicit monitor patch and a native DARK settings plan. Observe real updater exit; inspect/reboot saved state using resume console, require native config/settings and fresh mining/Core. Original browser retry must also run with current helpers. Agents still quota-blocked untilOct25; no independent review pass. Exact matrix remains79599rows/zero exact passes. Do not final.
- Full native-install workspace001 PASS92binaries337tests/zero failed/ignored; guide407links PASS. Source097 retains333files/library2b62d1e5b3af243db5ebc788f6013ea854dea4cd6216c23d2175daf523ab0a4a/32173864B. Backend078 remains pinned to096core, unchanged model behavior. No source changes after097 so far; work-only scripts/plans added.
- Actual install077 PASS exited0/307942ms/no truncation in work/simulator/qemu-s9/native-update-cycle-001/install-077. Uses exact same-release signed image469743...56e, backend078, explicit monitor patch, fixed4+guest ticket filter,1000H/s cap, two fresh private jobs, cycle006provisionedSD/MACf2:cd:e5:5e:93:fa, native DARK write plan. Native stop verified, signed validation HTTP200/36726ms, native step2 keep=1 HTTP200/478ms. Guest executes archived updater, reports writing SPL/UBoot/FPGA/FIT, Upgrade completed, native kernel restart188.724172s, serialEOF, actualQEMUexit0. Restart897bytes SHAb8098802293e566a73d045bca1d0555e64324802e51a4a02093020822de0ffc3. No supervisor kill or separately issued sync is counted as restart.
- Install077 snapshot schema3: raw e0755d8f35d562c4e165618825efd02fd74d60f086d6baf5fff0ef94c13de3ab/20512768B; gzip624ab575d8fdeb3317e54aaca5d51bbd548a3e98e7e9ffbf85e9e7d2c85adc1c/18950912B; guest_sync_completed=false with actual restart observation. Independent isolated inspection PASS in native-update-cycle-001/install-077-inspection. All four boot members exactly match signed update and baseline; uEnv remains ecc526...3f81/26B. Monitor override absent from overlay. Native settings are staged in upper/sysupgrade.tgz SHA c56e6973f0801725c099172adea3571639d66d364ca50eb01a9c779008629f1b/7465B, not yet restored under upper/etc. Next native boot must consume archive and prove settings/mining configuration readback. Do not infer persistence from staged tar alone.
- Install077 explicit session0 independent audit ACTIVE88470. Once finished, run two Core candidate tests. Then post-update boot078 with browser payload/current408retry, restore-state plan and console-readback.sh. Readback script derives cycle006resume, adds exact original monitor and four archived boot-file SHA checks. Do NOT reapply monitor patch on this boot: native updater must have removed it. No next boot/Core pass yet. Installation remains incomplete until this and second restoration verification boot pass. Full original board/remote/matrix gates remain open. Do not final.
- Install077 audit session0 PASS4682computed hashes/6ASIC replies/4submissions/two fresh jobs+clean+difficulty change. Both Core tests PASS in native-update-cycle-001/core-install077-{0,1}, height0→1 and restart persistence. Blocks02ae67d8029eb07e8e93845f08ec211a20bb6ed55d354e9daae635668fb1fe41 and0284885e015678bcda9b39bf8cabc05f6c34279fcc3528c81d69b6a09d059897.
- Post-update-browser078 FAILED exited1/360332ms/trace_truncated=true. Actual original monitor and all four boot-file SHA checks passed. Native boot restored the exact saved mining configuration and static network/MAC. Native HTTP returned webTheme=null; the required DARK persistence assertion FAILED. Browser correctly remained waiting for the failed HTTP probe; no browser retry success, mining/Core pass or valid snapshot is claimed. End-of-run19MiB compressed overlay's hexadecimal snapshot export exceeded the ordinary64MiB aggregate trace bound. Retainer then failed JSON EOF atcolumn26660226, so only raw result/logs, not derived dashboard/pool reports, were saved. Rootcommandlog boser-post-update-browser078.log. Preserve this failure.
- Cause established from actual077 native backup:36members includes/etc/bosminer.toml and/etc/config/network but omits/etc/web_storage.json. Original etc/sysupgrade.conf extracted isolated032 SHA9e87b05fe2f12c84d1c465b396322748479f64bc40e5bab061887f0bd95020a6/128B, allcomments. Original sbin/sysupgrade extracted033 SHA4e40954cb7d3fd98467eda6a0963dc7e5bcd579f229e4a7492997783971a2d09/6302B; add_uci_conffiles lines104..110 reads/etc/sysupgrade.conf and keep.d plus changed package config. This is a native default-policy omission, not silent compatibility or pass. Archive originals unchanged.
- Prepared explicit native keep-file configuration for cycle002: append only/etc/web_storage.json newline. Derivedsysupgrade.conf d7116cbec060fe94c01d21da5575e66d3141d8b0f20c5a53a854b0d524392cb9/150B. Work/native-update-cycle-002/console-initial.sh checks originalhash, appends via guestnativeprintf, checksderivedhash. keep-policy-adaptation.json records parent/derived/reason, pending execution. This is separate native configuration adaptation; it does not convert078 into a pass.
- Code now uses install contract/2. If a native Write plan requests settings persistence, inspect the actual downloaded backup and require its public settings before stopping mining or committing. Savedfileuses snake_case fields while APIusescamelCase; mappings explicit and values unchanged. Exact native null-file encoding is retained in dashboard-sd-inspection-006. Non-null enum spelling still requires actual newbackup evidence; do not normalize an unknown value to pass. Extended archive test checks extraction/public fields, omitted/changed settings rejection, private data not exported, corruptgzip rejection. keep-policy release-all001,strictClippy001,workspace001 PASS92binaries337tests, before subsequent trace/retention fixes.
- Attempted independent dark run0snapshot inspection at dashboard-dark-inspection-006 FAILED1811ms/exit1/no truncation:7zip reports Headers Error/warning1; parser also encounters non-entryWarnings footer and reports missingfield. Rawlisting retainedext4-listing.txt. No filesystem-consistency pass. Earlier native restart cycle remains its separate observed result. Need native journal replay/independent inspection before asserting this dirty snapshot's full inventory. Do not weaken parser to turn warnings into a pass.
- Latest source edits after097: saved SD runs reserve existing128MiB maximumtrace for large post-update snapshot exports. Firmware timing/filebounds unchanged. Snapshot retention failure now writes sd-storage-failure.json and continues independent HTTP/pool report retention; partialdisk remainsinvalid andfinalrun remainsfailed. These last edits need fullchecks/source098. No firmware currentlyactive. Next run cycle002 install079 with explicitkeepconfiguration andstrictpreflight. Then postupdatebrowser080 with exactoriginalmonitor/bootfiles andnativeDARKreadback+restore; secondbootverifyrestoration. Actual408browserretry and fullnativeupdatecycle remain open. No final.
- Source098 frozen333files/library2b62d1e5b3af243db5ebc788f6013ea854dea4cd6216c23d2175daf523ab0a4a/32173864B. Latest native-keep-policy clippy002,release-all002,fmt-check001,links002 and workspace002 PASS92binaries337tests/zero failed/ignored. Backend078 unchanged. Source098 adds install/2 backup preflight and128MiB for saved-disk runs plus error-retention fix. Source099 not created yet.
- Active install079 session92580 at work/simulator/qemu-s9/native-update-cycle-002/install-079. Exact command in tool history; same077parameters except source098helpers and console-initial002 (nativekeepfilepolicy) with write-state001/provision006 inputs. Rootlog boser-update-install079.log. Guest confirms nativekeepconfigured/MAC/miningconfig; nativebackup/preflight/restart pending. Preparedcycle002console-readback derives001readback and additionally checks configuredkeepfilehashd7116c...2cb9. No firmware executable patch forkeep; monitorpatchstillseparateinstall-onlyadaptation. Do not build heavy code while this validrunisactive.
- Recovered078 failednativeHTTPrecord verbatim fromrawguestlog into native-dashboard-failure.json. Nine actualrequests; passedfalse/errornative settings did not persist across restart. Decodedobservation6 into native-uptime-response.json confirms original22.02.4-plus/nativeuptime67/publicwebTheme=null. failure-source-identities.sha256 bindsresult/log/record/decodedresponse. This is diagnostic extraction, not a replay or pass. Entire078 stillfailed/truncated, no saved-diskreuse.
- Additional review finding for next mining-protocol work: native guest warns that pool rejection has both result=false and non-nullerror. stratum.rs submits all verifier failures withcode23; unauthorized/malformed alsoresultfalse. Duplicate/stale deserve distinct reference codes; nullresult error form is accepted by strict archivedJSONRPC. Tests currentlyassertlegacyfalse, s9_pool_work_audit and s9_submission_audit hardcode23+false. DO NOT silently changeverifier to acceptbothfornewruns: addversionedwirecontract andpreserveoldidentity replay iffixing. Primaryprotocolreference stillneeds exactpin; websearchfoundSlush/Braiins manual location andprimaryreferenceimplementations, no source downloaded/pinned yet. This remains an opencorrectnessfinding; no finalreviewpass.
- Install079 actual nativebackup nowincludes/etc/web_storage.json underexplicitkeeppolicy. Strictpreflight FAILED beforeminerstop orinstallation: APIDARK savednativeDark. This supplies actualnon-nullencodingevidence. Retainfailure; noinstallation/mining/Corepass. Minerwasstoppedbyownederrorcleanupafter144287ms, backendrecordsSIGKILLexplicitlynotrestart. Parentwaitsfor300spoolcompletion; active92580atlastcheck. CorrectedpreflightusesexplicitDARK→Dark,LIGHT→Light,null→nullenum mapping; retainsnativerawvaluesandstillrejectsmissing/changed/unknown/casevariants. Darkmappingdirectlyobservedin079;Lightrequiresitsownfutureactualtest. Noarbitrarycasefolding.
- Enumfix focused10testsPASS,strictworkspaceClippy001/release-all001PASS; previousfull337testsPASSbeforethissolehelperchange. Source099retains333files/samelibrary2b62d...0a4a. Actual install080 ACTIVE in native-update-cycle-002/install-080 with099helpers andsameexact079inputs/explicitnativekeeppolicy. Rootlogboser-update-install080.log; activeexecsessionfromtoolcall. Old079minerhasalreadyexited; onlyitssupportVMcollectionstillruns. Do notheavybuildwhile080validfirmwareactive. Next verifybackup/commit/nativeexit/snapshot, independentaudit/Core/SDinspection, thenpostupdatebrowser081usingcycle002console-readback+restore-state001withoutmonitorpatch. Thirdbootverifysettingsrestorationstillrequired. No final.
- Install079 terminalFAIL exited1/307871ms/no truncation. Nativebackupraw7821B retained asnative-backup.tar.gz SHA324230c82fa8005ffcb9b17331293ef877ddddb11783bf68c5dd174e3ef17a13, matchesHTTPbodyidentity. No updatecommit. Schema2failedsnapshot retained unsynced; do notuseaspassingrestoreinput. Dashboard/pool/packet evidence allretained.
- Install080 PASS exited0/307929ms/no truncation. Actualbackupincludesbosminer.toml2c4f...fbd/316B, networkf08a...640/417B, configuredsysupgrade.conf d711...2cb9/150B andweb_storage.json dc3580ea0ed51782dbb4462ad0fed6a55d29fe4cd64877bf42130bac0ae3bd0f/108B withnativeDark. Strictpreflight passes; native stoprefuses4028/uptimeadvances; signedvalidation37009ms; commitHTTP200/422ms; archivedupdaterwrites4bootfiles; kernelrestart209.542933s, EOF,QEMUexit0. Restartserial42b4aa1f9498fbc07bfc3f114475b1d05564e9886b6c86bc5567e1ce90f33d50/897B. Snapshotraw70bcdbeb9359055baf0b4d5ba1c58820e7cb93826fc01e38ec5c4ea1a8b925ab/20512768B,gzipf04501abd35bfac4c1d1ee268f141739c61501f6651e8a9d8096d6b16730ffd7/18951288B,schema3,native-syncfalse+realrestart.
- Independent080SDinspectionPASS atnative-update-cycle-002/install-080-inspection: fourbootobjectsmatchsignedimage/baseline,uEnvunchanged,nomonitoroverride. Nativependingrestorearchiveupper/sysupgrade.tgz608c43d809b68934474a18ae9c04c4924094102ac606f60be53148deda6ddf1c/7689B. Session0auditPASS4803hashes/4serialresults/3submissions/twofreshjobs+clean+difficultychange. Corecandidates0ead6cb9002f12bd5c673d3c4fbd40a043ae8727b346a3b3e225d72fbf05e290 and048f64eb017cbe64f6a67c83a9afb35cf81793b4181930f0636a3e886dc83b30. TwoCoretestsACTIVE incycle002/core-install080-{0,1}; toolcallsrecordsessions. No Corepassclaimeduntilresultschecked.
- Post-update-browser081 ACTIVE75510, outputnative-update-cycle-002/post-update-browser-081; rootlogboser-post-update-browser081.log. Uses080snapshot,cycle002readbackscript(originalmonitor+fourbootfiles+configuredkeepfilehash),restore-state001(DARK→originalnull),current408retryhelper,099boot/poolhelpers,128MiBtrace. No monitorpatch reapplied. Browserpreparationabout3min; Coreauditworkrunsduringpreparationbeforetiming-sensitiveguest. Next verifyactualDARKreadback+restore,browserpass,bothcookieprofiles/WS,independentaudit+Core; then thirdbootverifyrestoration. No final.
- Extendedexistingqemu_s9_backend pinnedinspector withslush-mining/slush-extensions sources. Rustbuild/fmt/strictClippyPASS andbothisolatedactualextractionsPASS. Source100freezes333files/same2b62d...library; onlyinspectorchangedafter099. work/simulator/slush-mining-source-001/references.json bindsarchive7de97fcbc2f3da0e5a65a7ad673c05a6bbe2f9cda9148726882f6164e12074f5,commitb2a24d7424784cada95010232cdb79cfed481da6,28entries. Extractedmining/service.py96415ed61f3801a875b4db08afeac8c46697e617defa958ad94e9bfb0744266a/6162B;lib/template_registry.py e2b2f3221e26fa345a7cfb3e4db6c98dc4c1ff301519fa3332779c6c4061d645/9333B;lib/exceptions.py9b82ca73762ca718015ac11c0c5d71655dfe39d386df4d7f3137f06f8d88b547/105B. ServerraisesSubmitException forunauthorized/unsubscribed/malformed/stale/duplicate/range/lowwork; inheritsstratum.custom_exceptions.ServiceException,transportdependency notyetpinned. No codeexecuted.
- work/simulator/slush-extensions-source-001 bindsarchive44325c134f86f94b84102a43f38d4a313ccb52084e76d4d2d08022b954aeaab4,commit04679d0a85e7fb0f64b9995d7192c966aa03466d. stratum-extensions.mediawiki d0f58f427403614bde8c805b144839bbea1d22df69a0780b22e44c09296d7d9c/11788B isextensionreference,notnumericerror-codeproof. OfficialJSONRPC1.0 response§1.2 https://www.jsonrpc.org/specification_v1 explicitlyrequiresnullresultonerror. NativeBraiinswarningmatches. Numericcodesstillneedexactprimarycontract; do notderivefromsecondarysearch. Poolwirefix/versionedauditor remainsnextimplementationaftercurrentupdatepath.
- Both080CorecandidatesPASS height0→1 andpersistedafterrestart incycle002/core-install080-{0,1}. Next081stillpreparingitsbrowserpayload(session75510), noin-guest resultyet. README/NATIVE-BOSER nowdescribe080configuredinstallationandtestedcommand, withpost-updategatesexplicitlypending. New simulator/protocols/stratum-v1-response-review.md recordsopenwiredefectandexactprimaryreferenceidentities; nofixyet. These documentationeditsareaftercheckpoint100. qemu_s9_backendinspectorcode itselfalreadybuilt/fmt/Clippy/actualextractionsPASS.

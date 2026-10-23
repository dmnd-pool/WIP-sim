# Active simulator checkpoint

Status: **incomplete**. Continue the implementation loop. Do not send a final
success response. The frozen v11 matrix still has79,599 rows and zero complete
exact physical targets. The original-board, full matrix, actual remote Boser,
fault/recovery and independent review gates remain open.

The [earlier continuation](continuation-2026-10-20.md) contains the requirements,
prior experiments, exact firmware identities, restrictions and backend fixes.
This record continues that work. Do not repeat completed research without cause.

## Current environment

- Work in `/home/user/src/md-sim`. There is no `.git` directory here.
- Archive originals are unchanged. Do not write through their external links.
- Host Internet sockets and socket send/receive calls are denied. Private Unix
  socket pairs with read/write work. Guest networks run in isolated VMs.
- All three agents remain blocked by their unchanged quota until25 October2026
  at08:48AM, with timezone unknown. Independent review is still incomplete.
- Use `CARGO_INCREMENTAL=0`. Development target directory is
  `work/simulator/check-target-085`. Release helpers are under `target/release/`.
- Do not run large builds during a timing-sensitive firmware test.
- Never print raw large result JSON, snapshot markers or packet-capture markers.
  Use selected `jq` fields. Saved-run results can exceed150MiB.
- Emergency shell session70212 remains available if the tool wrapper hits the
  per-user temporary-file quota. Root storage still has substantial free space.

## Native update cycle002

All directories below are under `qemu-s9/native-update-cycle-002/`.
Backend078 remains pinned to source096. Its executable hash is
`a3f5fccbb9cfd362e552a7e2c41a7086604e42cf40093a18458c5ae79e88a7fc`.
The private pool uses backend003. Firmware is the archived Braiins22.02.4-plus
SD image and signed update whose full identities are in the earlier record.
This is still an adapted reference environment, not an exact physical board.

- `install-080`: PASS, exit0,307929ms, no truncation. Explicit native keep-file
  configuration preserves `/etc/web_storage.json`. Native backup preflight,
  signed validation, installation, kernel restart and actual QEMU exit0 pass.
  Independent saved-disk inspection passes. The audit verifies4803 hashes,
  four serial results, two fresh jobs, a clean transition and difficulty change.
  `core-install080-0` and `core-install080-1` both pass acceptance and restart
  persistence. Block hashes are0ead6cb9002f12bd5c673d3c4fbd40a043ae8727b346a3b3e225d72fbf05e290
  and048f64eb017cbe64f6a67c83a9afb35cf81793b4181930f0636a3e886dc83b30.
- `post-update-browser-081`: PASS, exit0,357342ms, no truncation. It boots the080
  disk without reapplying the monitor patch. Original monitor and four boot
  hashes, mining/network configuration, MAC and native keep-file policy pass.
  Native API reads DARK, restores null, and reads it back. Actual Firefox login,
  assets, telemetry, WebSockets and separate profiles pass. The second profile
  receives native AuthError on its first request. The permitted408 retry was
  not needed. Audit:52652 hashes,56 serial results/submissions, two fresh jobs,
  clean transition and difficulty change. `core-post081-0` and `core-post081-1`
  pass acceptance and restart persistence. Blocks0dbcc4ac0107757c6fa2f2930f2e38504ed522f3d35a6e5608c601c990544ed5
  and03750c7802340694fb0594b5cfe8bea24dfab4f10fabed1aa4de5d5e1ef25242.
- `restored-settings-082`: PASS, exit0,237505ms, no truncation. Boots081 disk;
  native API observes the restored null settings and original boot-file checks
  pass. Audit:26317 hashes,29 serial results/submissions, two fresh jobs, clean
  transition and difficulty change. `core-restored082-0` and
  `core-restored082-1` pass acceptance and restart persistence.
  Blocks08fef91301321407533690c2e48be68ecdc178dcc69bbb609f8cfaf1a37e27a6
  and041436df035c0086967a072a38bdf884c9548ca8924aa16f9eccf434336b97f5.

Run082 snapshot raw hash is
`d71033d3743bcd2b71cb3c3148eeb01a9f35e6cffe9db8f4e996fcf52d1e792a`,20643840B.
Its gzip is `847f8755389aa6e9d3caca3a9e564ad10d2be997923c4275c9486bff835f5a24`,19036041B.
Schema2 records actual guest sync followed by owned emulator termination.
It does not claim a native shutdown or crash durability.

`post-update-browser-081-inspection` is **FAILED**, exit1, no truncation.
7zip reports `Headers Error`; the ext4 listing has `RECOVER` set. The parser
rejects its warning footer. Raw listing is retained as `ext4-listing.txt`.
Do not call this a consistency pass or ignore the warning. A clean native
shutdown and independent inspection are the next storage work. Existing
sync/kill snapshots must retain their original declared scope.

## Stratum response correction

The pinned SRI source is now inspected in `sri-stratum-source-002/`.
Archive SHA-25642d1742ca392944fe1ca1462bb65bbbb2544877dd0e80e33c74a7294aa448233,
commit31f80744907c6e21f4ae491ca6a0d86797bce8b0.
`sv1/src/methods/client_to_server.rs` is35519B, SHA-256
9b66a2ebe30d5a18306468e28067ae03fd74aa58e2eb4f4ee2130630ae71d081.
It establishes codes20 through25 and null results for rejected submissions.
`sv1/src/json_rpc.rs` establishes the three-element error array. No acquired
source was executed. See `simulator/protocols/stratum-v1-response-review.md`.

Implemented explicit ResponsePolicy in pool configuration and reports.
New frozen pool-input schema5 uses `json-rpc1-stratum-v1`; schemas2–4 retain
`legacy-false-code23`. Schema5 keeps schema4 per-session seed derivation.
Unknown schemas fail. Distinct responses cover malformed20, absent/stale21,
duplicate22, low difficulty23, unauthorized24 and not subscribed25. Denied
authorization still returns false with a null error. Wrong submitted worker
names are unauthorized; non-string names are malformed.

Both S9 wire auditors bind the report policy to the exact frozen pool input.
Their response table is independent of the runtime encoder. Mutation tests
reject the other format, wrong codes, missing fields and wrong data. Actual
socket tests cover both explicit contracts and negative cases. The current
auditor successfully replays old run080 into
`install-080/regtest-session-0-response-replay.json`. This is replay, not new
firmware execution. Run082 was also audited with the new verifier under its
old frozen schema4 contract.

Full workspace tests:92 binaries,338 tests, zero failed or ignored.
Release-all001, strictClippy001, fmt001 and link check pass. Source101 freezes
334 files and library22f5c8c9960e44011b3a9e670507848d0aa0c2f20e4cf2b94c32e6d58c3685ee/32192530B.
The final non-string worker classification fix came after101. Affected mining
tests002, release-all002, Clippy002 and fmt002 pass. Source102 freezes334files
and library5ab9e35e90aa3d68e4a5972409fc2d1992b4087d222b3d00b9b0dac8c47fa0e5/32192802B. Logs are under
`checks-2026-10-21-001/stratum-response-*`.

Fresh `qemu-s9/stratum-response-083` PASS: exit0,237877ms, no truncation.
Pool-input schema5, actual selected session0 uses json-rpc1-stratum-v1.
Its wire trace has nine code23/LowDifficulty errors, all with null result.
Audit passes23635 hashes,21 serial results/submissions, two fresh jobs, clean
transition and difficulty change. Both Core tests pass acceptance and restart
persistence in `bitcoin-core/stratum-response083-{0,1}`. Block hashes:
01eb57db03577a5b4592aae61ba1d9d78bf58d31fc7fdbd43a92872d62d145be and
02abe20c03adb8933a4f14f2ae92a669aee4cafc706466d68b897dbb5dacf6d2.

## Native reboot snapshot work

Implemented `sd-native-reboot` as an explicit boot-runner option requiring
saved SD storage. It cannot override the installer's restart. SerialGuest
writes `/sbin/reboot` without `-f` and uses the existing bounded serial reader
to require a kernel restart, complete EOF and actual emulator exit0. The
request has its own marker and hash. No kill is used on a successful reboot.
Schema3 records this separately from a guest sync. Independent filesystem
inspection is still required; an observed reboot alone is not a consistency pass.
Old sync/owned-termination behavior remains explicit in run metadata.

Own-process tests check the exact non-forced command and reject a process
that refuses to restart. These unit fixtures are not firmware evidence.
Full workspace339tests/92binaries PASS, zero failed or ignored. Release-all,
strictClippy and fmt PASS in `native-sd-reboot-*001.log`. Source103 freezes334
files/libraryff405128e470fe4c2ba8cbae54007d669e4bce23f9f3f1d627bfcd2b2d25fadb/32228474B.

Actual `qemu-s9/native-sd-reboot-084` PASS: exit0,237560ms, no truncation. It uses103helpers,
083saved disk, original monitor, cycle002 readback console, cycle001 verify
state, backend078/pool003,1000H/s/fixed4, schema5 difficulties-28/-27, save-sd
and sd-native-reboot. Root log is `checks-2026-10-21-001/boser-native-sd-reboot084.log`.
The guest takes its normal shutdown path. Kernel restart occurs at124.725946s,
serial reaches EOF, and QEMU exits0. The404-byte serial observation has hash
086e3b33c4de07469f7f37b8d7e6b26abe0ac5aa9e15c46826b7e8bb578d0cf1.
Snapshot schema3 raw hash1d14a689eb1d6e1c3067757ced15dadf7fcb80c4b3c7b07ee5ea6211ee5365c8/20643840B,
gzipc3f7c8a80e411586a47faca9a779da049b7463f53873447b38fe8efad767cdd2/19036015B.
`native-sd-reboot-084-inspection` PASS with the unchanged strict inspector.
It records69 overlay entries and all expected boot hashes. Restored null theme
file is058f68f4b34213aec4a82e77940af8aab78afacd88d611afd28d060ed5d0920d/106B.
The monitor override is absent. Ext4 partition hashdf7480f82297ce72a29d59ae75f496c899ef47b99b7aea26d3ec7133936dc288/67108864B.
Audit:33001 hashes,26 serial results,24 submissions, two fresh jobs, clean
transition and difficulty change. Both `bitcoin-core/native-sd-reboot084-{0,1}`
PASS acceptance and restart persistence. Blocks032589eb007422aeb50ff2d0fc46e927b4860facbe48e4a5bd86e593b349e3bb
and0543f7290386868c90b09408839cc69f761a95a8893ab1b986bf4f77f52f85c8.
The earlier081 inspector failure remains failed. Normal shutdown supplies a
separate successful disk result. No e2fsck repair or warning bypass was added.

## Next action

The automatic cycle extension is now implemented but **not yet executed**.
Changes after103:

- `s9_dashboard_cycle` accepts `signed-update=ORIGINAL_MONITOR_SOURCE` and
  `browser-payload=MANIFEST`. Signed update requires the browser phase.
- It freezes helper binaries, backend manifests/executables, Core, the console
  template and optional inputs. It rejects changes before each phase and at
  completion. Do not rebuild release helpers while a cycle is running.
- Provisioning and non-install phases request native reboot. Install phase0
  uses the native installer's own restart. Every disk receives the existing
  strict independent inspection. Boot validation now requires schema3 plus
  matching actual restart serial/exit evidence. Its JSON bound now matches
  the existing128MiB trace ceiling and hexadecimal expansion.
- New `support/s9_cycle_update.rs` reuses the monitor-adaptation code and
  isolated signed-update inspector. It generates explicit keep-file setup
  and post-update checks for the original monitor and four baseline boot
  files. It rejects injected member names and invalid hash text.
- The existing HTTP completion checks moved unchanged from pool_vm into
  `s9_update_validation::verify_completion` for shared use. Existing negative
  tests still run through the pool VM helper.
- Native service and browser reports must exactly match raw guest records,
  and their execution/specification hashes must match the actual run.
- New cycle uses an explicit1000H/s cap and a frozen wall-clock epoch. It
  retains a fresh theme/MAC and new pool entropy for each phase. Phase1 runs
  the native browser. Each phase still gets independent mining replay and
  two Core acceptance/restart tests. Fault/recovery and remote gates remain open.

Focused tests001 PASS. Full workspace001 PASS:92binaries344tests, zero failed
or ignored. Release-all001, strictClippy001, fmt001 and link check001 PASS. Logs use prefix
`checks-2026-10-21-001/native-update-cycle-`. Next strictClippy/fmt and source104
have completed. Source104 freezes335files and the unchanged103 core library.
The complete cycle is ACTIVE in session71822. It started at19:34UTC and writes
`qemu-s9/automated-update-cycle-001`. The root log is
`checks-2026-10-21-001/boser-automated-update-cycle001.log`. Do not modify its
release helper binaries while it runs. The executing command is:

```
target/release/examples/s9_dashboard_cycle \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/backend-078-ubsan \
  /tmp/md-sim-qemu-pool-003-ubsan \
  work/simulator/qemu-s9/automated-update-cycle-001 \
  signed-update=work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin \
  browser-payload=work/simulator/network-vm/native-driver-024/payload.json
```

The browser phase still has the explicit360s host wall bound. Prior081 used
357s with sync/owned termination; the added native reboot can expose this
bound. If it fails, keep the failure, inspect its exact cause, then adjust
only a justified host observation budget. Do not weaken native assertions,
ordinary guest timings or mining deadlines. The actual shutdown084 passed
without the monitor adaptation, so do not add that patch to later phases.

1. Observe the active automatic cycle and inspect each completed phase.
   Preserve any failure, fix its actual cause and rerun affected checks. The
   new schema3 validation and larger bounded result reader are implemented.
2. Require all three native phases, browser, strict disk inspections and six
   Core results before documenting the cycle command as verified. Keep the
   install-only monitor patch distinct from later original-script execution.
3. Continue update/recovery faults, actual Boser remote access and the remaining
   specification. The six blocks above and one firmware family are milestones.
   They do not complete the requested matrix or original-board gate.

## Automatic cycle001 failed; provisioning readiness fix

The cycle above finished with failure, not success. Provisioning returned
exit0 in39402ms and observed a native restart at29.304464 guest seconds.
The strict disk inspection then failed with `ext4 entry field missing`.
Its retained 7-Zip listing has `Headers Error` and `RECOVER`. No test phase,
browser phase or Core test ran. See `automated-update-cycle-001/assessment.json`
and `disk-provision/ext4-listing.txt`. Keep this failure unchanged.

The provisioning console finished while native startup was still active.
Native run084 records `procd: - init complete -` after the S99 miner startup.
The original `/etc/init.d/done`, extracted in `original-init-done-034`, calls
`mount_root done` at START95. SHA256 is
967c80f7667784b6b61e7469e854bb69e6322eec28ee0fd4fdac027226a8ef89.
Original `/etc/diag.sh`, extracted in `original-diag-037`, defines a no-op
`set_state`; SHA256 is
7ddfd79fab415628758173dc652c1b31e0114cc4f9d8c7cf3533f802085b17f2.
Both members came from ramdisk2bd49c70...4247154b3 with isolated extraction.
Failed extractions035/036 found no rcS/halt member and remain recorded.

The cycle provisioning script now waits for the actual procd completion log
with120 bounded one-second guest polls. It then rechecks the written uEnv.
The host observation bound is180s, increased from90s to allow native startup.
This does not run or replace native init services. The disk assertion remains
strict. This cause and fix still need an actual new run to confirm them.
Next: format, focused test, lint/build, freeze source105; run cycle002.

Source105 freezes335files. Release-all002, full-workspace002 (92 binaries,
344 tests, zero failed or ignored), strictClippy002 and formatting PASS.
Automatic cycle002 is active in session95063, started19:42UTC. Same command
as001 with new output `automated-update-cycle-002`; log
`checks-2026-10-21-001/boser-automated-update-cycle002.log`.
Do not rebuild release helpers while it is active.

## Automatic cycle002 failed; native startup race remains a fault case

Cycle002 timed out at180047ms with no truncated trace. The native procd
init-complete event occurred and uEnv was verified. Normal shutdown then
interrupted the default miner's PIC initialization. Trace for address57 shows
reset07, reads at0f80, then power-disable15/00 without an intervening jump06.
The PIC model returned Mode error4 at97302988396ns and stopped the VM.
No native restart completed. No disk or mining pass is claimed for002.
This shutdown-during-initialization behavior remains an unresolved fault case.
Do not change the loader grammar without evidence of its command behavior.

Original `/etc/init.d/bosminer` was isolated-extracted into
`native-s9/original-init-bosminer-038`; SHA256
4f13a361f3ccc1ba40d592fd8e90b8fde490311dc2db27ab19c728c9e337c215,
5828bytes. It enables autotuning and public pool defaults on a blank SD boot.
Existing private-pool runs instead set explicit private pool and fixed-frequency
configuration before this native START99 service. No public network exists in
either experiment. The archived2019 power.rs also documents reset/read/jump
before normal power-disable, but is not proof of the2022 closed binary's full
shutdown behavior.

Source after105 changes provisioning to use that same existing private-pool
console path. It waits for native init completion after the real mining
observation window, then requests native reboot. All later phases require the
saved mining configuration with `cmp`; they do not overwrite it. The first test
phase still configures the network after native preinit handles the changed MAC.
The provisioning boot now has independent fresh-work audit and two Core tests,
so the cycle requires four actual mining boots and eight Core results. Host
budget360s matches the existing mining runs. No guest delay or PIC rule changes.
The driver contract is now schema3 and shares one mining-verification function.

Disk inspection now explicitly rejects 7-Zip WARNINGS/ERRORS and RECOVER flags,
even without a warning footer. This strengthens the same assertion and gives a
clear error. Negative tests cover each condition. Failed001/002 remain intact.
Next: affected tests, lint, build, source106, then automatic cycle003.

## Cycle003 and stronger filesystem checks

Source106 contains335files. Full workspace003 passed92binaries/344tests,
zero failed or ignored. Release-all003, strictClippy003, fmt003 and links003
passed. Cycle003 started19:49UTC in60755 and has finished with failure at its
strict disk check. Provisioning itself returned exit0 in237470ms without
truncation. It used the private mining path and reached kernel restart at
147.425346s. Restart serial b47c823ae3f8de7e5dd15aa80ba5ad6dac08c5159a22fa47c6d14317f8f001a2,
404bytes. The new independent audit passed and two Core checks are active;
these do not turn the failed cycle into a pass.

The new RECOVER check also rejects the earlier084 snapshot. Earlier084's
inspection PASS established parseable file data with no7-Zip header warning,
not a clean ext4 state. `native-sd-reboot-084-inspection-002` retains this
new failure. `automated-update-cycle-001/disk-provision-recheck-001` also
fails with the new clear error. Neither old evidence set was rewritten.

Exact original shutdown evidence is now extracted:039 `/etc/init.d/umount`
(38e2453d73021489099cb17dbd9076b3096a12e401e7aeb123517f44f98c1bbc,
106bytes) calls sync then `/bin/umount -a -d -r`.040 `/etc/inittab`
(8b395f7d827753e1e92d4a1c369d9fe16caa23bf742b3f78e4599c4dd9b1b6e3,
106bytes) assigns native shutdown to rcS K shutdown. The original root
inventory includes K99umount. Native restart still leaves RECOVER set.
Its exact cause is not yet established. Do not claim clean unmount.

The inspector now captures dumpe2fs and e2fsck and always runs a full
read-only `e2fsck -f -n`. An explicit optional `replay-journal` mode first
runs `e2fsck -p -E journal_only` on its disposable extracted partition.
The installed pinned manual says this option only replays the journal and
performs no further repairs. Require exit0, then full read-only check exit0,
unchanged partition hash, and the existing strict7-Zip checks. Keep raw and
derived partition identities separate. The firmware's saved snapshot is never
modified or replaced by the inspection copy. This mode is under test and is
not yet selected by the cycle. It measures recovered consistency, not raw
clean shutdown. Next: run it on084 and003, investigate any actual failures.

WarpAxe follow-up discovery found commit link
9cbc497187b27e72a71b5a3136de068b1d290adb, but pinned source/API and main source
pages were unavailable through the provider. No source bytes were obtained.
Recorded provider response: `bitaxe/warpaxe-discovery-002/provider-response.json`,
SHA256800b05e0d1797d293850bc8f7497fdd6bc47f957a2938f9ab0b6c767c850cd50.
No model behavior changed from this description. Archive search found S9 PIC
applications and unrelated APW12/PIC1704 files; none proves an S9 loader contract.

Journal inspection results:084 `native-sd-reboot-084-journal-001` PASS.
The derived partition5845e3a9b8de73c799d7f7732bcb248be5c3399cec466408b4c0135519cf8b78
has69 listed entries. Raw partition remainsdf7480f8...7133936dc288.
The raw084 full-check run still FAILS. Cycle003's journal inspection FAILS:
after committed replay, free-block count26726 differs from26698, and free-inode
count16366 differs from16332. 7-Zip still reports Headers Error. No repair or
exclusion was applied. e2fsck1.47.4 returned0 despite these declined corrections;
the inspector now additionally requires the five-pass output without extra
diagnostics. A regression uses that actual counter-error text.

The original baseline partition inspection in sd-bundle-006/boot has no7-Zip
warning and no RECOVER flag. Its original hash is
663b345610b7aecd9843f8a96fc5dc5c1d630b18f218b5f70812129af4d9acae.
Original rc.common041 confirms shutdown calls stop. Thus the native K99umount
path is present; its exact failure to close this mounted extroot is unresolved.

Core cycle003-provision-0 and-1 both PASS acceptance, height0->1 and restart.
Audit:35153 computed hashes,30 serial replies,28 submissions,two jobs,clean
transition,difficulty change. Blocks0cbef5c00207ab53af6fd360da0aee00cf513fa3c04f5289309c49b8bb2e4fdc
and035e00e80173e877cb1f4d044d18ea92b7d7394de9e63e6f219da5f4c8a7fcff.
These are separate mining passes; cycle003 remains FAILED.

Diagnostic native-storage-checkpoint-085 is now ACTIVE in session29371.
It boots the unmodified003 snapshot, uses its captured console-initial script,
then explicitly asks the original guest mount tool to remount /overlay and
/tmp/mnt/boot read-only. It requires both actual /proc/mounts entries to be ro,
then uses normal native reboot. This is an explicit storage-maintenance
operation, not a claim that unassisted shutdown cleans the filesystem. No
original firmware executable is patched and no saved input is repaired.
Console: qemu-s9/console-native-storage-checkpoint-085.sh. The script has an
outer set-eu scope so a failed earlier subshell cannot be masked by later work.
Backend078, source106 boot helper,1000H/s,fixed-filter4, pool schema5, frozen
clock1792612748, original monitor, and native verify-state(null) HTTP probe.
Log: checks-2026-10-21-001/native-storage-checkpoint085.log.
Next: inspect actual mount/reboot outcome, then strict raw filesystem check.

The085 console hash is5944309c10a9c41984dca4c60945f38ab65ade577cb5060d5d1f482717e80c63.
Its parent cycle003 console-initial hash is
f362ceb20372172e6b382c272ff8a6f0b3a14b617aa102a15fad696cd0934ec9.
The maintenance remount is a new explicit diagnostic operation. It is not a
model fix, general repair, original firmware patch or unassisted shutdown pass.

Read-only checker tests002 PASS3tests, strict workspaceClippy002 PASS.
Release inspector build002 PASS. New actual084 journal002 and003 journal002
checks are running in8645/26673. They enforce diagnostic-free e2fsck output,
not just its misleading zero exit. README and NATIVE-BOSER now distinguish
084's earlier file-inventory pass from recovered consistency and raw failure.

## Native read-only remount085 works; cycle integration in progress

Run085 PASS, native remount at111.184108s and reboot at119.813251s.
Both /proc/mounts entries changed to ro. Restart serial
b0071e311ba1d340b705fbacd87b9e535e1108ed05e4d514456efbd310f1b4e2/404bytes.
`native-storage-checkpoint-085-inspection-001` PASS with no journal replay,
no image mutation and no general repair. Raw and checked partition hashes
both equal a8928b89e3f72cc1e2f321bb8754942cfbd392bce9705d1b555f47df8fd9601c;
45entries. The archived baseline also passes a strict raw check in
`sd2022-baseline-fscheck-001`; it matches663b3456...14d9acae exactly.
Inspector gained explicit `baseline` mode for that independently checked input.

085 mining auditPASS:26881hashes,27serial results,25submissions,2fresh jobs,
clean transition and difficulty change. Core checks active24829/78656 for
06015a6500e9d5ea0a1061a8df8bddea79444d95367d42585baa78ab35d311a5 and
07f2a31502eb42a5c9b9bc5d8648f08086c7fc52f91e2e8f7e525e845369cd1c.

Source after107 adds `sd-read-only-checkpoint` to the boot runner. It requires
native reboot, excludes installer operations and records its actual script
identity and scope. The runner waits for successful native HTTP completion,
then performs the native remount after browser checks. It verifies both mount
states, and still requires a separate native restart plus strict disk check.
A negative regression rejects absent, failed, wrong-origin or duplicate HTTP
completion. The console stop metadata now describes native success correctly.

Cycle schema4 selects the checkpoint for provisioning and normal phases.
It binds the actual checkpoint record to console.json. The install phase keeps
its native installer restart and selects explicit journal replay on a separate
inspection copy. Every other phase requires strict raw filesystem consistency.
All restart inputs remain the untouched snapshots, never the inspection copy.
Eight Core results are still required. The original unassisted shutdown and
startup-interruption cases remain open, not excluded or relabelled.

Focused tests5312 are running; first release-all001 build passed before the
last help/adaptation metadata addition. Source108 is not yet frozen. Next:
finish tests, lint, rebuild final helpers, freeze108, run automatic cycle004.

Source108 is now frozen (335files). Final release-all003 and strictClippy002
PASS. Clippy001 found a nested if; it was simplified without changing behavior.
Full workspace001 and affected boot tests002 are being checked. fmt002PASS.
Both085 Core nodes PASS acceptance and restart persistence.085 runtime exit0,
237629ms,no truncated trace. Baseline raw inspection and084 recovered
inspection pass;003 still fails the stronger actual diagnostic check.

Automatic cycle004 is ACTIVE in session71729, started20:08UTC. Uses source108,
backend078 and pool003; all helper binaries are frozen. Do not rebuild release
helpers while it is active. Exact command is the previous s9_dashboard_cycle
invocation with output `work/simulator/qemu-s9/automated-update-cycle-004` and
unchanged signed-update and browser-payload paths. Log:
`checks-2026-10-21-001/boser-automated-update-cycle004.log`.

It requires provisioning mining plus the3 native phases,8Core results,
raw filesystem checks after explicit native read-only checkpoints, and recovered
inspection after the installer's own restart. It remains a reference-environment
suite. The physical original-board, complete matrix, full fault/recovery and
current Boser remote publication gates remain open. The earlier adapted LuCI
remote suite does not test this exact Boser run. Review agents remain unavailable
until their recorded account limit resets; no new independent review occurred.

Cycle004 provisioning has passed the strict raw check and both Core tests.
The provisioning partition is0c34ef54ee86920a8615cc8aa01d92e322636a9fece052c1f18bb36a862fabe4.
Signed install phase run-0 is active. Final source108 checks:full workspace
92binaries347testsPASS; affected boot tests00218testsPASS after the lint-only
simplification; fmt002,clippy002,release-all003PASS. No C changes this cycle.

Independent work while004 runs: new Rust example `s9_update_fault` prepares
one controlled FIT payload-bit fault in the exact archived signed update.
Archive parsing and mutation stay inside the preparation namespace. It checks
both the full original archive and exact FIT hashes, retains all other bytes,
and exports a separate artifact plus original/derived hashes and offset.
A full byte comparison rejects extra, absent, wrong-mask or out-of-range changes.
Test001 and debug build001 PASS; scopedClippy001 and isolated preparation001
are being checked. No native rejection test has run yet, so its gate is untested.
These builds use only check-target-085; release helpers for004 remain unchanged.
Do not build workspace release until004 completes.

## Damaged signed update rejection086 active; cycle004 post-install check passes

Automatic004 run-0 native installer completed all four writes, restarted at
177.635119s and exited0 in308289ms without trace truncation. disk-0 passes
explicit journal-only replay on a disposable partition followed by the strict
read-only full check. The saved restart input is not repaired. Both run-0 Core
checks passed. run-1 is preparing the actual browser environment at20:22UTC.
Do not rebuild its eight frozen release helper binaries while71729 is active.

The fault preparation001 passed in its isolated namespace. The derived image
is98a5748d67fd98e4be0e54fb752f82620163f16b848cbdd0c6ee1ddf89b6ef02/19179888.
Exactly one XOR01 at archive offset9168902 changes FIT byte177 to176; every
other original byte, including signature trailer, is unchanged. Manifest:
`native-s9/tampered-signed-update-001/manifest.json`. Preparation alone is not
native rejection evidence.

Source109 adds a separate RejectTampered action, exact original/derived input
checks and `dashboard-update-corruption=IMAGE`. It uses the same original LuCI
upload handler. A pass requires the actual unsupported-format page, unchanged
session and no install step2 or flashing response. It then starts native mining
and requires fresh jobs. It records `rejected_update` separately from
`valid_update`. The generic HTTP error does not identify which native validation
check rejected the image; do not claim signature-specific error evidence.
Negative regressions reject an absent rejection, install controls, wrong parent,
wrong image, changed fault and missing restart. Focused tests001 and scoped
strictClippy001 pass. New source snapshot109 retains all source files and the
unchanged built core library. Debug helpers were built separately to preserve
cycle004's frozen release binaries. No model/C change.

Run086 ACTIVE in32526,20:22UTC. Exact command:
```
work/simulator/check-target-085/debug/examples/s9_boot_bundle boot \
 work/simulator/original-zynq/sd-bundle-006 \
 work/simulator/qemu-s9/native-update-rejection-086 spl sd2022-v1 \
 backend=work/simulator/qemu-s9/backend-078-ubsan budget=360 \
 presence-pin-fixture=e0 fan-fixture pic-contract asic-contract asic-packets \
 local-pool pool-vm=/tmp/md-sim-qemu-pool-003-ubsan temperature-fixture \
 adapt-fixed-filter-bits=4 hash-cap-hps=1000 pool-difficulty=-28,-27 \
 regtest-genesis dashboard-probe dashboard-luci \
 dashboard-update-corruption=work/simulator/native-s9/tampered-signed-update-001/fault.tar \
 adapt-clock-epoch=1792614130 \
 adapt-monitor-fan-source=work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin \
 console=work/simulator/qemu-s9/console-private-pool-014.sh
```
Log checks-2026-10-21-001/native-update-rejection086.log. No firmware rejection,
mining or whole-run pass yet. Next: inspect actual086 response and resumed
mining sessions; run independent audits and two Core checks per completed
pre/post maintenance session. Also finish004 and inspect all its phase gates.
All review agents remain account-limit blocked untilOct25 08:48; no new
independent review is available. Original physical board/full matrix/current
Boser remote/fault completeness gates remain open.

086 FAILED before miner startup: support kernel reports initramfs unpack write
error, then missing libgcc_s and init panic. Runtime exit1 in7686ms, trace intact.
Its debug-tool base filesystem contains103730663file bytes, compressed51331719;
the non-browser support VM has256MiB RAM. This did not test firmware rejection.
No guest deadline, firmware check or assertion was weakened. Optimized helpers
were built independently in `work/simulator/rejection-target-109/release/examples`
(build001 PASS12.07s) to preserve004's frozen binaries.

087 ACTIVE41672. It repeats086's command with the independently built optimized
`s9_boot_bundle`, output `native-update-rejection-087`, epoch1792614360.
Log checks-2026-10-21-001/native-update-rejection087.log. Actual original firmware
has reached native networking startup. All source109 workspace checks pass:
fmt001, strictClippy001 and93testbinaries351tests,0failures0ignored.
Source109 contains336files; core library remainsff405128...25fadb/32228474bytes.
004's browser support VM has now started successfully; firmware startup is
pending its browser preparation. No complete004 or087 pass yet.

## Native damaged update087 passes; shared worker execution implementation

087 PASS exit0,307995ms,no truncated trace. Native upload19,180,318bytes
(including multipart framing) returns6947bytes in11608ms. Rejection record
passedtrue,commitfalse,install_controls_presentfalse. It then starts actual
native mining and observes fresh sessions0 and2. Both independent audits PASS:
shared trace17630hashes,11serial results; session0 has3submissions,session2 has7.
Each has2fresh jobs,clean transition,difficulty change. All4Core checks PASS
acceptance0->1 and restart persistence in
`bitcoin-core/native-update-rejection087-s{0,2}-{0,1}`. Exact blocks:
0/0 0fa17912013758571eedd62180d79aeddfd0b534d8498ca63e1560b3f44e8c89
0/1 0106b98f017ab4bbe82828e24d289f84ca13a7a5ace0aa6f0802d3247962bdd0
2/0 057c9c2801b6aa72c5e17b491ccb5d42e490e63e0e423c6d9a608e322af7ebb1
2/1 06390c5b0270fb2858f73df6e1a9bcd3888f99537dd79d99878045e4925de7bb
README andNATIVE-BOSER document actual tested command, scope and evidence.
Links001PASS418checked0broken. Source109 full workspace93binaries351testsPASS.

004 run-1 browser phase PASS356431ms,exit0,no truncation. Actual Firefox
native login/assets/live HTTP/WebSockets/two profiles PASS. Its native read-only
checkpoint plus native reboot led to a strict raw disk check PASS, no replay or
mutation. Partition e55c16f23ae0507424c9d00f0e3feabf8a15417294147b857a8904ec557ae8ec.
The final run-2 is active. Overall004 assessment not yet complete at20:34UTC.

Source110 adds `s9_execute_prepared PREPARED NEW_OUTPUT local|worker-ipc=PROGRAM|worker-ssh=CONFIG`.
It freezes the full prepared ProcessSpec, all runtime objects, adaptations,
console/pool metadata and reporter build into an existing LockedRun. It uses
existing LocalExecutor, PrivateWorker and SshTransport/WorkerExecutor; no new
worker protocol or replacement firmware. Scope stays ProcessPrimitive with an
explicit archived reference-board description. No resolved physical target,
original-board, mining or dashboard gate is inferred from transport completion.
A repeated prepared seed is the same execution identity, not new fresh entropy.
Local/worker use the same extracted `s9_execution_report` implementation.
Read-only pool session selection moved to `s9_pool_report` so replay does not
pull in live pool execution code. Selection still preserves the first authorized
session even if it failed. Actual shader/header/Core checks remain separate.

Strict scopedClippy004PASS after fixing redundant borrows, then separating the
reporter from live pool code and using a single shared module import. Failed
lint logs001-003 retained. Focused tests001PASS(19boot+9replay tests).
Optimized source110 helpers built in `work/simulator/worker-target-110/release/examples`
(build00111.38s). Real md-sim-worker-ipc built under the same release directory
(ipc-build0015.37s). Canonical release helpers frozen for004 remain untouched.
Source110 checkpoint exists; source/library hashes in its manifest.

Preparation110 ACTIVE95212 at20:34:47UTC. Uses the087 flags plusprepare-only,
newoutput `qemu-s9/worker-prepared-110`, helper `worker-target-110/release/examples/s9_boot_bundle`
and epoch1792614887. It generates a fresh frozen pool seed. Log:
checks-2026-10-21-001/native-worker-prepare110.log. Next: replay that exact prepared
process locally and through the separate local IPC worker; verify destination
hashes, results/cleanup, native negative upload, mining before/after and Core.
This will not be an SSH/TLS or external physical worker test. Those gates stay
open. Do not rebuild any helper while its frozen execution is active.

## Automatic signed cycle004 PASS

Overall assessmentpassedtrue at20:37UTC. Four actual boots:provision,write/install,
restore/browser,verify. Final run-2exit0,237527ms,notruncated. Final strict raw
partition778cd5bbd31df9e4c732368f5b8d6fef84ab493c37e1db7ef9a29dbd4c2516e9;
no replay or mutation. Install disk-0 requested journal-only replay on its
inspection copy, but its hash did not change:fed2dd094ba708791e0c2c16b0ab4abf87c9060acdc1a26e274f8db0cd813079.
Thus replay was permitted by policy but made no mutation for that actual disk.
Do not state that a repair was required on004's installation output.
All8Core checks accepted0->1 and persistedafterrestart. Per-phase audits:
provision30446hashes22serial18subs; install7801hashes4serial3subs;
restore41077hashes29serial28subs; verify29572hashes27serial23subs.
All4have2freshjobs,cleantransition,difficultychange. ThemeDARK,MACd6:1b:f8:99:cd:b9.
Physicaltarget/originalboard/fullfaults/currentBoserSSH/TLS gates stillincomplete.
Canonical release helper freeze for004 has ended; retain their source108 identity.

Source110 workspace checksPASS:94binaries360tests,0failed0ignored; strictClippy001,
fmt001. Preparation110 completed with ProcessSpec e5d6f6458ec2e94deeacd942e80b0faed16e3f341524824063759acf07ae1984/11428bytes.
Actual local replay088 ACTIVE57646 and IPC worker089 ACTIVE59491. Both locked
run hashes equalf2bc75c5313bc5f350caeb81eb5055c01260f18b96031b92c8e7471ae4573f0f,
54input entries. Worker transfer destination verification complete; actualrunID
run-a1e4a283718bd2d6e3761af9143122a72f6345b4bff44759e71803a044d0b6de.
Worker-owned live stdout/stderr are under worker-reference-ipc-089/worker-state/runs/RUNID.
LocalExecutor retains its final output; this helper does not yet export a local
live trace file. No SSH/TLS or externalhost claim. Next:finish both executions,
audit actualsessions before/afterrejection, runCore and check worker cleanup.

Local088 process/native suite PASS,exit0,307769ms,no truncation. Its real mining
sessions are0 and3 (the restart allocates afterstop_session2), not087's0/2.
Both actual session audits are now running (68735/29256 were087; the new sessions
are recorded in tool state). Select these IDs from this run's native record.
Worker089 is still executing. Do not infer its pass from088.

Review found that the generic LocalExecutor did not retain a live output file.
It now uses the existing bounded execute_with_trace path to save
local-stdout.log and local-stderr.log in the attempt directory. An actual
isolated shell regression observes both streams before cancellation, then
checks their exact result bytes. All9queue tests PASS19.76s, including worker
loss/reconnect/result cleanup and resume evidence checks. No guest/network
policy changed. Source111 checkpoint has339files and library
4ae8623ae68177450b65432c846d29a0c5ac0f209e681e1a43b2d8de8950f76f/32233856bytes.
Existing backend078 and running088/089 still use their frozen original builds;
no emulator or helper binary was overwritten. Full source111checks69257 active;
strict workspaceClippy001PASS. README/NATIVE-BOSER now document004 fully;
guide links001PASS420checked0broken.

## Paired local/IPC reference execution passes; final affected replay starts next

088 and089 both PASS actual process and native rejection/resume suite. Runtime
088307769ms;089308452ms. Both exit0 without trace truncation. Independent audits
check sessions0 and3 from each run.088shared15560hashes6serial;089shared16102hashes
9serial. Each session has2jobs,cleantransition,difficultychange. All4Core checks
for088 and all4for089 PASS0->1 and restart persistence. These are the same4candidate
hashes from the same preparedwork, with eight separate node executions.
Reports:bitcoin-core/worker-reference-{local088,ipc089}-s{0,3}-{0,1}.

Worker transfer verifies52distinctobjects from54lockedinputentries. Retrieved
result109112953bytes SHA fed17563be2e9b159fd339fb395383d7519adf2f066d7611b2518ad3670a42cb.
Actualhashmatches cleanupreceipt; runremovedtrue,published_inputs_retainedtrue,
token_receipt_retainedtrue. The run's old live stdout/state files are removed
by acknowledgedcleanup; use retained guest.log/backend.log andresult.json now.
No SSH/TLS/externalphysicalhost claim. The same-hostworker execution is real.

Source111fullworkspacePASS94binaries361tests,0failed0ignored;strictClippy001,
fmt001PASS. Review then added the missing explicit result.tools comparison to
LockedRun.backend_tools in s9_execute_prepared. Existing088/089 observed tools
match their lockedinputs (independentjq comparisontrue forboth). This assertion
is now in code; scopedClippy005PASS. Allchanges areRustonly.

Source112 checkpoint is being completed with the worker evidence documentation.
Build112 in separate `worker-target-112` is ACTIVE52329. It builds only the updated
replayhelper. Next: use112helper on the same prepared110input for local090 and
IPC091 (workerprogram remains frozen worker-target-110/release/md-sim-worker-ipc).
This exercises final live-output and stricter tool-identity behavior. Waitfor
local090 locked-run.json before starting091: the source Store is single-writer
locked while copying; two simultaneous freezeoperations would conflict.
Existing089/088 results are retained and do not substitute for affected replay.
Read this record after compaction. Do not send a final completion message:
zeroexactphysicaltargets stillpassallgates. CurrentBoserSSH/TLS, originalboard,
fullfaults/recovery/fullmatrix and independentreview remainopen.

Source112 helper buildPASS9.39s; guide linksPASS423. Affected replay090 active
61596 since20:48UTC,091 active84477 since20:49UTC. Source110 prepared process and
workerbinary unchanged; helper is worker-target-112/release/examples/s9_execute_prepared.
090actual live local-stdout.log exists and advances during firmware execution.
112finalfocusedtestsPASS9,fmtPASS. Source112339files/librarysame4ae8623...0f76f.

New source113 work: `support/execution_cancellation.rs` converts SIGINT/SIGTERM
sent to this coordinator into the existing runtime cancel flag. It records the
first signal and monotonic observation time, then lets the executor retain
actual result bytes and perform acknowledged worker cleanup. It does not send
signals to unrelated processes. `s9_execute_prepared` reports requested
cancellation separately from process success and records watcher-write errors.
The handler itself uses only an atomic store; filesystem writes run on its
bounded watcher thread. Actual SIGTERM-to-flag/record test passes in a separate
child process; all10replaytestsPASS and scopedstrictClippy001PASS.
Source113 checkpoint/build running; buildtargetworker-target-113 onlyreplayhelper.
090/091frozen112helpers are not overwritten.

Next concrete test:run113helper with the real prepared110firmware viaIPCworker,
newoutput worker-reference-cancel-092. Use one parent shell to retain its owned
child PID. Wait for this run's own worker-run.json and a live native miner-control
stop observation in worker-state/runs/ACTUAL_ID/stdout.log. Retain that exact
observation, send SIGTERM only to the owned coordinator PID, wait for expected
nonzero exit, then assert result.status=cancelled, complete trace, recorded
signal15 and acknowledged cleanup. This tests cancellation during actual native
update validation; it is not a completed mining or recovery pass. No such test
has run yet. Start only after113buildpasses. Alsofinish090/091and their real
sessionaudits/Core checks; selectsessionIDs fromeach actualdashboardrecord.

## Cancellation092 PASS; read-only preparation ingress114

Actual092 cancellation test PASS. The owned parent shell waited for this run's
live native miner-control stop observation, savedcancellation-trigger-observation.log,
sent SIGTERM to the owned coordinator PID, then required a nonzero CLI exit.
All5independentjq/sha checks returnedtrue and the shell exited0 (session74511).
Actualrecordsignal15 at134097ms aftercontrollerwatchstart. Runtime statuscancelled,
exitnull,122133ms,notruncated. Worker result10405803bytes SHA
30e8f72653f3ef5e072e8d7a8246c67c514ad6643c44a36677a4289d1dd2da1b,
acknowledgedcleanupremovedtrue. RunID
run-ae3b4162cfa525be2a18d9df2606463875ada95553806d93ebe339c95bc785e9.
No mining-completion or recovery pass is claimed for this intentionally stopped
run. Raw result, trigger, signal record and receipt remain in
qemu-s9/worker-reference-cancel-092. Driver logchecks-2026-10-21-001/native-worker-cancel092-driver.log.

090(local112helper)PASS307842ms;091(IPC112helper)PASS308042ms; both exit0/no truncation.
Actualsessions0/3 inboth. IndependentauditsPASS.090shared17431hashes7serial,
submissions3/4;091shared16224hashes5serial,submissions2/3. Allsessions2jobs,
cleantransition,difficultychange. Actual live090stdout equalsretainedguest.log
hashc4074858209651dfa4c2a41a6eb339eaa8c3b08985087d541011c8ca726c2bcd;
stderr/backend both86a52fbb53b55e194781131fea239fe6c09804ca332078cd12e60f8a955770e9.
090Core4sessions69613/96287/78513/4526 running/completed;091Core4nowactive
66621/30556/60915/26535. Check individualblock-acceptance.json before claiming.

SignalfeaturefullchecksPASS94binaries362tests,0failed0ignored;strictClippy001PASS.
The child signaltest now captures its own output and requires1testpassed;
a renamed/missing test cannot silently pass by executing zero cases.

Source114 also removes a writable Store::open on the prepared input store.
Each selected object is read through validated real directories, a regular
single-link file and exact size, then Store::import checks its source hash and
completed destination under the output store's full quotas. No source lock,
permissions change or writable source mount is needed. This also permits
simultaneous frozen-input readers and a read-only remote deployment input.
Regressionchecksunchanged0555/0444permissions,exactdestinationabc hash,rejected
hardlink/symlink,andchangedcontent. Scopedtests001PASS11,Clippy001PASS. Test002
onlyaddspermissionrestorationforitsownTempDircleanup; rerunningbefore114freeze.

114checkpoint/buildACTIVE12479. Buildtargetworker-target-114 includesboot,
replay,pool,isolatednetworkVM andmd-sim-worker-ipc. Next:prepare fresh114input
with the087negativeupload flags, newpoolseed/currentepoch, then run a complete
noncancelled actualworker case to exercise the cancellation watcher idle path
andread-onlyinputingress together. Do not reuse the intentionally cancelled092
as a normal firmware or mining pass.

Next larger unmet transport path: actual SSH board execution in the existing
isolated Linux VM with separate client/worker namespaces. Existing
remote_namespace_check alreadytestsactualSSH/controltransfer, butitsnative
mode is the older LuCIprocessfixture, notthisboard. Reuse that setup; add a
separate board-process mode which invokes s9_execute_prepared worker-ssh from
its client with a captured preparedinput read-only at/etc/md-sim-board. Transfer
all runtimeobjects overSSH to the existing UID1000worker. Two QEMUs will then
run inside the outer LinuxVM; measureactualperformance andresourcebounds before
claiming success. Use capturedpayload support for the~200MiBinputcache; do not
mount hostfilesystem/network. Retain compressed exactworker-result bytes via
bounded outerVM stdout export (rawresult~109MiB; gzip likely~20MiB), plus
lockedmetadata/transfer/cleanup. Hostcanreconstructlogs from the exactProcessResult
and run independentaudits/Core. This is a design direction ONLY, notimplemented.
NativeboardSSH/TLSgatewayaccess stillneeds its own selected-run forwarding;
performing SSH execution alone must not claim that dashboard gate.

### 21:16 UTC: full worker114 and framed SSH transport

Preparation114 completed. Process specification:
`b18de761c371329e96eddef6c870f0c46b95f800301d0ecd7e920cfc3f5f9c01`,11428bytes.
The exact command in the preceding entry ran as IPC094 with the114helper and
114worker. It passed in308321ms,exit0,complete traces. The cancellation watcher
stayed idle. The native damaged-upload rejection and miner restart passed.
Actual mining sessions0/3 each had2jobs,clean transition anddifficultychange.
Both independent audits completed successfully. Shared trace:13159hashes,
5verifiedserialresults. Four separate Core checks accepted the firmware's blocks,
changed height0to1,and preserved the block after restart:
`bitcoin-core/worker-reference-ipc094-s{0,3}-{0,1}/block-acceptance.json`.
Block hashes, in session/candidate order:
`0ce72196014bdfbab97dc40da6ed8c9cff52bfa982bab83945287a00ea17b45c`,
`02a63f7a02bbfac3580d0e2636ae8364c967c734cb49035df6462a85dc967189`,
`08ad9f1801bc097940c9c0525f60a8dd83debd14ddacc6d3a436337639631dde`,
`074b300500e305fbfdb49d4c37444173098e209176c1789c437c4549ce555c39`.
Worker cleanup acknowledged exact result
`a47836a2148c3ca2488c1639729ebbf5d50944469d575e279a810f99a2a3d004`,106359765bytes;
removed run storage and retained the input cache/token receipt.
All8Core checks for090/091 also completed successfully. Their repeated prepared
input produced the same four candidate identities as088/089, not new seeds.

SSH previously created one connection per256KiBrequest. Added an explicit
`SshTransport::new_streaming` with one owned OpenSSH child, bounded newline frames,
per-request deadlines, pinned executable/key/trust checks, process limits and
no automatic operation replay. The remote relay accepts `--stream`; ordinary
one-shot callers remain supported. On I/O/framing/worker error the channel closes.
Source fixtures still use ordinary negative host-key/timeout checks; successful
artifact/resume/execution/result/stop/cleanup checks now use the framed channel.
Frame tests1PASS; SSH transport tests7PASS. Actual isolated namespace suite115
and its independent host report PASS:
`network-vm/ssh-stream-115/remote-report.json`.
This remains a transport protocol fixture. It is not native board access.

Source116 freezes342files and corelibrary
`cd7c8385501fafe9f8724124ba32f899c0ae3efb16858e78ba74c127d5239bf0`,32252332bytes.
The original078device backend is unchanged. New342file checkpoint includes
Rust worker relay/coordinator support changes. Full strictClippy003PASS;
board helper tests002PASS20. Clippy001/002 retained: unusedunit and needless
questionmark, both fixed. Full workspace tests001 are running in79009.

New board SSH path reuses the actual frozen ProcessSpec and s9_execute_prepared.
Preparation command, tested:
`work/simulator/ssh-target-116/release/examples/remote_namespace_check prepare-board work/simulator/qemu-s9/worker-prepared-114 work/simulator/ssh-target-116/release work/simulator/network-vm/scenarios/namespaces-001.json work/simulator/network-vm/board-driver-116`.
It captures exact input hashes in a read-only payload. The worker's separate
mount namespace masks that payload; only the client can read it before transfer.
Worker UID1000 receives inputs over authenticated SSH. The existing namespace
fixture tests remain separately labelled. Large exact worker results use bounded
gzip export with compressed and uncompressed hashes. The host report checks the
captured input/worker/tool/coordinator identities and cleanup acknowledgement,
then retains original guest/device/pool records through the shared report reader.

First actual board SSH attempt116 is ACTIVE62366:
`work/simulator/ssh-target-116/release/examples/isolated_network_vm work/simulator/network-vm/board-ssh-116 work/simulator/network-vm/board-driver-116/scenario.json`.
Log:`checks-2026-10-21-001/board-ssh-vm116.log`.
The outer support VM has3072MiBRAM,2vCPUs,900swall,1440sCPU,128MiBtrace.
The inner firmware's existing360sProcessSpec and all seeds/clocks remain unchanged.
Do not claim SSH firmware success until actual execution, independent audits
and Core checks complete. Native board dashboard SSH/TLS forwarding is still open.
All3review agents still report the account limit; no independent review pass.

### 21:22 UTC: SSH116 failure isolated and guard corrected

116 FAILED before firmware execution. It transferred52distinct objects,
230008417bytes, over the framed SSH channel. The worker ran the selected
process for118ms and the guard rejected `/proc/irq`. Exit1 was retained and
cleanup acknowledged result35a97f6a21b3c98886a6ac92dc99cf18ea6f36177ec5b398f8285c733b657a0a,
798bytes. This is NOT a firmware boot pass. The outer VM used134239ms,
174011446ususerCPU,1535900ussystemCPU,1343628KiBmaximumRSS. Its guest
scenario failed honestly; the outer process exit alone is not a passing result.

Added exact failed-result extraction, tested:
`work/simulator/ssh-target-117/release/examples/remote_namespace_check extract-board-failure work/simulator/network-vm/board-ssh-116`.
The extracted `board-result/` keeps result, input metadata, backend error,
transfer and cleanup records. Export verifies both compressed and complete
uncompressed identities and grants no gate.

Added a real worker mount probe to the SSH transport suite. It runs the same
captured shell through the nonzero worker UID and retains `/proc/self/mountinfo`.
Probe117 and its host report PASS:
`network-vm/worker-mount-probe-117/remote-report.json`.
The derived worker-mountinfo.txt hash is
8d1a5f4cd476520688cd22d00921369492cc3a0526d33966376d877f03fc22d3.
It shows exact read-only proc submounts `/proc/sysrq-trigger`, `/proc/irq`,
`/proc/bus`, all from the same private proc filesystem and parent mount.

Corrected confinement to accept only those three measured protections. It
requires their exact proc root, private parent mount ID, same filesystem device,
proc type, and ro/nosuid/nodev/noexec. Writable mounts, other roots, foreign
devices/parents, or missing flags remain rejected. Tests use the literal
measured row and negative mutations. This preserves protections; it does not
remove or relax a namespace or allow arbitrary directory input mounts.

Source118 checkpoint:342files, corelibrary
a80b0c15e79774861cde4cba9e7c7d6d5a9b996e68a1904d42359fa60e571e84,
32241550bytes. Backend078 remains unchanged. Clippy118PASS. Tests001for116
completed94binaries376PASS,0failed0ignored. Full118tests active92707.
Source118 helper also reports the actual selected board worker ID while running.
It keeps the fixture gateway scope separate from the board execution scope.

118preparation is running with the complete087negativeupload arguments,
new epoch1792617760,prepare-only,source118build. Lognative-worker-prepare118.log.
Next:prepare-board with source118 and this preparedinput; rerun actual SSH board
execution and a local/IPC comparison with this same process and report identity.
Do not reuse114or116results as passes for the changed execution identity.

### 21:33 UTC: local118 passed; next SSH prerequisite isolated

Prepared118 ProcessSpec:c72b378b219b99104e7396d7444900da9a9aad85069f100ee2b31c85ce72470e,
11428bytes. Source118IPC095 passed exit0,307988ms,complete traces.
Locked-run hash:3d125614d15640f0c3bd73b36bbee987cc7797801ba4e940210fade6301f932b.
Native damaged-image rejection and restart passed. Actualsessions0/3 each
passed independent two-job/clean/difficulty-change audits. Sharedtrace11568hashes,
11verifiedserialresults. Four Core checks pass in
`bitcoin-core/worker-reference-ipc095-s{0,3}-{0,1}/block-acceptance.json`.
Candidate hashes:
0cae5df8021cdcee383b8f9cc277f8f38873085693824d7ea82f50f88c45be5e,
06c41b91018a25a903d7fbf5f0d6077a00ae247e15b26620e6c550c28d7d8251,
004b645e01d9eb9f817f40a1c0fd04daf90de562fc8fb13ed54f370744befc8c,
044419ea0003808ccc12221a29093ed3acf031be13bfa43250bca6497b571534.
Full118tests:94binaries397PASS,0failed0ignored. Fmt/strictClippyPASS.

SSH118 FAILED at the next prerequisite: the pool support VM did not report
ready within20seconds under nested software emulation. The firmware was not
started. Guard accepted the measured proc protections. Inner exit1,20327ms,
complete trace. Cleanup acknowledged31683ecfee414de7eb1427b5c84ab29790c1bd60f95c6953ad5b886843a90f53,
2162bytes. Extracted with118helper into`network-vm/board-ssh-118/board-result/`.
Outer usage:158070mswall,204506660ususerCPU,2293756ussystemCPU,
1585288KiBmaximumRSS. It remains a failed scenario even though the VM process
ended with0 after its guest init failure. No firmware/remote dashboard gate.

Source119 changes:
- Bounded SSH sessions rotate only after a complete response and before the
  next operation at the65536request limit. No uncertain request is replayed.
  SSHtests8PASS. This fixes otherwise failing long-running sessions.
- Progress polling waits for complete selected-worker metadata. An incomplete
  create/write cannot abort actual execution. Final evidence remains strict.
- Failed coordinator deadlines requestSIGTERM, allow35seconds for existing
  worker cancellation/result cleanup, then kill only the still-owned child.
- Gzip export rejects excess expansion, trailing members/data and hash changes.
- Pool startup failures now retain actual bounded kernel/backend diagnostics.
  New`pool-startup-seconds=20..120` selects only a support readiness deadline;
  default20 remains. It is recorded in ProcessSpec and adaptation metadata.
  Browser startup keeps its separate330second contract. Firmware clocks,
  hardware timing, mining assertions and total process budget stay unchanged.

119checkpoint has342files. StrictClippy119PASS and scopedstartup/export tests
PASS. Separate119release build contains all six helpers and actualworker/relay.
119preparation ACTIVE24957 with previous118fullnegativeuploadargs plus
`pool-startup-seconds=120`,fresh epoch1792618364. Lognative-worker-prepare119.log.
Next:run119overSSH, inspect actual support startup logs/readiness, then firmware
results. Continue exact local comparison and independent per-session Core tests.
The first two SSH failures must remain retained. Neither establishes mining.

### 21:50 UTC: third startup failure, existing pipe pool, result reader fix

Prepared119:0bc52eabec77ed74c67bdcd9e81c68c2a50e48f4d97dd9fae82a7f1e8d20da5b,
11473bytes. Its legacy_io metadata records120s pool startup. Total process
remains360swall/330sCPU/3GiBaddress space.119fulltests94binaries403PASS,
fmt/strictClippyPASS. Corelibrary119:
7af97cfd66a355dc697993e5e2fc8808cd707d6d211bf58fd8a43ccb4c97faaf,32241438bytes.

SSH119 FAILED after120361ms. The support pool VM emitted the actual Linux
`virt/tdx: TDX not supported by the host platform` line at its40.102734s,
but did not reach the Rust init/pool ready marker. Its backend stderr was empty.
The startup failure record now retains both actual streams. Firmware was not
started. Extracted complete records live at`network-vm/board-ssh-119/board-result`.
No further unchanged timer retry. Existing pipe-backed pool is the next path.

LocalIPC096 with this same119preparedinput PASSED308473ms,exit0,fulltrace.
Native rejection/restart PASS. Actualsessions0/3 both have successful independent
audits,2freshjobs,cleantransition,difficultychange. Sharedtrace19911hashes,
11serialresults. Core checks started as39542/70460/56063/44866 at21:49UTC;
confirm fourblock-acceptance.json files before claiming. Reports are
`bitcoin-core/worker-reference-ipc096-s{0,3}-{0,1}`.

Preparedpipe120 uses existing source119flags, with`local-pool` and no pool-vm,
dashboard,updateorwall-clock adaptation flags. Same original SD/SPL,backend078,
monitor adaptation,1000H/scap,filter4,difficulty-28/-27,regtest andconsole014.
No new miner or API model is substituted. The existing restricted libslirp
guestfwd sends the real Stratum bytes over command pipes to the existing
independent pool. It avoids another full x86 support kernel nested under TCG.
The omitted dashboard/update suites remain required; this isolates the SSH
firmware/mining milestone first. Their local coverage is retained separately.
ProcessSpec48c0dd345bb75da78b48e70720c1fd53eaf23c8bb6c2f70e0e647b96424b128c,
9634bytes. Preparation at`qemu-s9/worker-prepared-pipe-120`.
ActualSSHcase ACTIVE78045:`network-vm/board-ssh-pipe-120`,driver`board-driver-pipe-120`,
logboard-ssh-pipe120.log. It uses119helper/worker binaries. Selected actualworker:
run-db9793eb22acdcd36ca2c2c25509b5fc48f3d33dda04ba90c0beeca4c4a884cf.
Transfer/executionstage at81992ms. Do not claim boot/mining until results/audits.

Review found a separate real transfer cost: ResultData rehashed the entire
result on every256KiBchunk. A109MiBresult required about45GiBof repeated hash
input. Added a one-entry bounded worker result reader. It hashes each unchanged
completed file version once, opens withO_NOFOLLOW, rejects nonregular/hardlinked
files, checks inode/device/size/mtime/ctime and metadata after each range read.
Replacement or detected mutation requires a new complete hash. The receiver's
full downloaded checksum and worker cleanup acknowledgement remain mandatory.
The metadata check is an early change check; the final full checksum remains
the content-integrity condition. No result or mining gate is inferred from
cached metadata. Tests use the standard million-a SHA256vector, check actual
hash-read byte counts, same-length mutation, wrongidentity,replacement,links,
growth andbounds.2testsPASS;11real isolated worker testsPASS;strictClippyPASS.
Newsourcefile:`crates/md-sim-runtime/src/worker_result.rs`.

Source121 checkpoint/release active94573. Source120numberis unused for a code
checkpoint; pipe120 is a run made with119.121includes only result reader change
since119. Build createsnamespace/transport/network/replayhelpers andworkerbins,
not a replacement boot/pool image. Next test121SSHtransport and real board result
retrieval with that build after inspectingpipe120. Full workspace121stillneeded.
Live remote coordinator log files are another unmet workflow: worker Logs API
works, but WorkerExecutor polls onlystatus. Consider adding bounded stream
lengths to Status and downloading only appended bytes, then comparing retained
prefixes with the complete verified result. Do not add two empty log RPCs to
every50mspoll. This is a direction, not implemented code.

### Continuation: pipe120 CPU exhaustion and explicit nested execution profile122

CoreIPC096 checks completed: 4/4 independent isolated nodes accepted the actual
firmware candidates, changed height0 to1, and retained the block after restart.
Source121 full tests completed without a reported failure; strict Clippy passed.
Its real SSH fixture also passed at`network-vm/ssh-stream-121/remote-report.json`.
This tests transport and result retrieval, not a native board dashboard.

SSHpipe120 FAILED. It executed archived SPL, U-Boot, and Linux into early procd.
The inner process used342612ms wall; waited-child CPU was320503512us user plus
15361715us system. Its emulator was killed under the330s CPU allowance before
console activation. Last actual guest milestones: eth0 link168.19s, overlay
format185.07s, extroot213.23s, procd early220.755s, watchdog220.811s. No boot
completion or mining pass is claimed. Complete failed evidence is extracted
under`network-vm/board-ssh-pipe-120/board-result`. Outer wall444958ms, CPU597459706us
user plus3330921us system, maximumRSS1912948KiB. Selected worker run:
run-db9793eb22acdcd36ca2c2c25509b5fc48f3d33dda04ba90c0beeca4c4a884cf.

Source122 adds explicit`nested-tcg budget=1200`, restricted to the untraced pipe
pool. It records1200s inner wall,1190s CPU and900s pool boot allowance. Each
actual pool connection still has at most its frozen230s session deadline.
No guest clock, bus timing, ASIC filter, hash cap, firmware instruction, or
acceptance assertion changes. The enclosing local disconnected VM records1800s
wall/3600s CPU, client1560s and server1680s; inner limits remain separately set.
Both profiles are in frozen process/scenario identities. Scoped tests44PASS,
strict ClippyPASS. Full tests ACTIVE31793. Source checkpoint122 saved343files;
release build ACTIVE96170. The initial build command used a nonexistent example
name`s9_pool_vm_init` and failed before compilation; corrected to`s9_pool_guest`.
The next action is prepare/run122 over actual SSH and inspect original firmware
and fresh-work evidence. No completed exact physical target or release claim.

### 22:10 UTC: source122 running; live coordinator output implemented

Source121 full tests:94binaries405PASS. Source122:94binaries406PASS,
0failed0ignored. Full122release helpers were built from the root checkpoint.
The first deployment preparation failed because the relay binary was absent:
`-p md-sim --bins` does not build coordinator binaries. Saved incomplete
`board-driver-pipe-122/payload.json` remains. Built coordinator bins directly
from frozen122source with an explicit absolute vendor directory, so newer live
log edits could not enter122. Complete driver is`board-driver-pipe-122b`.
Prepared ProcessSpec764a965241613a769bff4d2df1e6d4723435a6396469b76fb2fa3b81ee602da1,
9690bytes. ActualSSH122 ACTIVE86407 at`network-vm/board-ssh-pipe-122`.
Selected worker run-51206ac55e0278eb0b2d01b99dcbe2099373ba2c0c7ebd28e9d22124964f737a;
execution stage82012ms. No boot/mining result yet.

Source123 adds bounded coordinator live logs. Worker Status reports current
saved stream lengths. Coordinator fetches only appended bytes, at most one
chunk per stream per status iteration, so backlog cannot hide cancellation.
Logs are provisional. Before worker cleanup, each retained prefix must match
the completely downloaded and hashed ProcessResult; remaining bytes come from
that verified result. Bad ranges, run IDs, streams, oversized/shrinking logs,
links and changed prefixes fail. No empty log requests on unchanged lengths.
The worker's full result and cleanup hash remain required. Local files are
worker-stdout.log and worker-stderr.log. Recovery retains and checks prefixes.

Two new protocol/file tests PASS. Actual private-worker tests observe both
streams before cancellation and compare final bytes. Changed live output
causes failure and keeps the worker result. The first test failed because its
five-second deadline included artifact transfer and executable hash checks;
actual execution had only1153ms. Fixed the test to give setup its own15s bound,
then require live bytes within5s. This changes no production execution limit.
Coordinator10unit+10integration tests PASS. Strict workspace ClippyPASS.
New realSSH coordinator fixture uses the same path, both streams, actualcancel,
fullverifiedresultandcleanup. It remains a protocol fixture, not firmware.
Frozen123release built. ActualSSHfixture ACTIVE12221 at`network-vm/ssh-live-123`.
Fullworkspace123 ACTIVE85909. Next inspect fixture and firmware results.

Review after123 found cleanup acknowledgement was saved under the completed
receipt name before validation. Current workspace now retains the raw response
separately, validates exact run/result/removal, then saves worker-cleanup.json.
A real-worker reply-mutation test checks rejection and recovery through the
actual durable token receipt. Test ACTIVE16110 (check current tool session).
This fix is not in frozen123. Independent agents remain account-limit blocked
until2026-10-25; independent-review gate stays incomplete.

Source123 full workspace:94binaries409PASS,0failed0ignored. RealSSH123 PASS.
Report`network-vm/ssh-live-123/remote-report.json`, execution identity
c979034229c8be1b1191ee41eb5383dc69eeb4cc8f097d4071d412604c04b5b5/9830bytes.
Actual common-coordinator live case lockedrunbc089c59b30314fd5d334e5973de597530563a677b4d17d5a8b1a66ebf350a7c,
worker run-c4ae4294c5b86956c233f266a098b2e414cf2cf8bf808452e9c0704e175c8cac.
Both streams observed before cancellation; final cancelledresulthash
4fb8af7cdc96c9d06b8e6bb89e0bfcc0806f60492f823c23aad05fa0d57d6da1,770bytes.
Exactcleanup acknowledged. No firmware/native-dashboard gate. README and native
command guide updated;427localguide links PASS. Cleanup reply mutation/recovery
testPASS26.72s (actual session45892, correcting the prior mistyped16110).
Source124 freezes the cleanup acknowledgement ordering fix plus these docs.
Fullchecks and release build active; inspectremote-live-*-124 logs.122stillactive.

### 22:16 UTC: final live-log checks and identical-process local comparison

Source124 full tests:94binaries409PASS,0failed0ignored; fmt/strictClippyPASS.
SSH124 PASS after the cleanup ordering fix:
`network-vm/ssh-live-124/remote-report.json`, identity
4a7c85d4e34f509a3b26e5cafe821b8d8cb154d858d99a5633f4f5b526e61abe/9830bytes.
This is still a protocol fixture, with actual live coordinator output/cancel,
not native dashboard or firmware acceptance.122boardremainsactive.

Current source adds`prepare-board-local` to the existing board driver. It
runs exactly the prepared ProcessSpec with LocalExecutor as guestUID1000 in
the isolated Linux VM. This is needed because the already measured host
restriction denies libslirp's AF_INET socketpair for the pipe pool. No repeated
host attempt. The same helper/build/input can be tested overSSH and locally.
The driver records its executor in its captured execution identity and v2
result record. Host report binds that field to the scenario, requires exact
local helper/build/tools, and rejects conflicting worker records. SSH keeps
its worker/hash/cleanup checks. Legacy v1 evidence remains readable asSSH only.
Local output has local-result.json; it is not labelled a worker result.
The separate SSH/TLS protocol fixture still runs, but cannot grant a remote
board result for local mode. Added provisional live-byte progress for selected
runs; byte counts do not establish boot or mining. No125execution yet.

BM1397 reference check: the saved provider view hash501c855b43ecf49a37575766454d1e4e439d0911f1b0c2f5bd2bef287642fe67
and exactESP-Miner driver9bd28dff43cb3c07a63c4843dba2704f7e071b1bfc7fb310cfd925cb35be2f56
were rechecked. Ordered-clock enable still lacks established behavior beyond
register reset and writes; no artificial clock success or newmodel was added.
Mujina contract explicitly excludes BM1397's different host-midstate format.

Source125 build/fmt/strictClippy and23board-driver testsPASS. Fullworkspace125
finishedPASS (count log before next report). Local125ACTIVE58332:
`network-vm/board-local-125`, driver`board-driver-local-125`, preparedpipe122
unchanged. Local lockedrun7c3ed5fb95137abb39afd5aa56980c80ebe53abe498ee2847eb4c6f78039efeb.
Live output grew to3356stdoutbytes and1833stderrbytes at133s. No boot/miningpass.
SSH122remainsactive. Source125's first generic observation string still says
'overSSH' even inlocalmode. Its capturedboard_local flag, v2executor field and
host report distinguish local correctly. Current source fixes that text and
generic error labels; it is a change after125, not silently applied to125logs.

### 22:23 UTC: worker preflight and bounded live diagnostics

Source125 full workspace94binaries410PASS. Source126 includes the local-scope
text correction and a worker capability preflight. Probe now advertises
status_saved_log_lengths. New coordinator rejects its absence before transfer
or execution, instead of discovering it after starting an old worker. The
capability is part of executor identity.11real queue integration testsPASS,
including an actual Probe reply with the feature removed and no later request.

The board harness now exports at most2048actual tail bytes per stream every30s,
with selectedrun, offsets and provisional/diagnostic-only labels. This makes
long nested boots observable without a guest control port. It does not change
the process, logs, boot conditions, clocks or acceptance checks. Complete
result hashes still establish final trace identity. Source126 checkpoint/build
ACTIVE65605; fulltestsACTIVE57408. StrictClippy126PASS. Next runSSH126 on the
same122preparedprocess to observe its actual boot while checking the current
coordinator. SSH122andlocal125 stillactive; preserve their outcomes.

Source126 fullworkspace94binaries411PASS,0failed0ignored. StrictClippyPASS.
Release build finished. ActualSSH126 ACTIVE46468:
`network-vm/board-ssh-126`, driver`board-driver-ssh-126`, same preparedpipe122
ProcessSpec764a965241613a769bff4d2df1e6d4723435a6396469b76fb2fa3b81ee602da1.
At22:25UTC the separate126VM was transferring artifacts. Local125ACTIVE58332
had~22KiBstdout at444s; SSH122ACTIVE86407 had starteditsworker22:06:30UTC and
is approachingits1200swallguard. Next poll/extract122 withitsown122helper,
inspect actual failure/success, audit selected freshsessions andCore only if
mining evidence exists.126live diagnostics are provisional hexsamples, not
manufactured stage markers. No required exacttarget or releasegate closes.

### 22:31 UTC: SSH122 failure and actual proc isolation defect

SSH122 FAILED after inner1179367ms,exit1,fulltrace. It reached actualguest
console, executed monitor/config scripts, and started network configuration.
No pool session completed or started. Lastkernelmessages were bridgeforwarding
atguest637s. Backend observedSIGKILL; waited-child CPU1099904906ususer plus
61374406ussystem. This is not enough to attribute the kill definitively to the
1190sCPUlimit. Outer wall1281285ms,user1747293160us,system16723069us,RSS1973980KiB.
Fullfailure extracted by122helper into`network-vm/board-ssh-pipe-122/board-result`.

The diagnostic read a parentVM nginx process under the child's numericPID7.
A new realprocess regression reproduced the proc namespace mismatch locally:
shellPID2 vs/proc/self/statPID67. Original launcher createdPIDnamespace with
unshare while Bubblewrap --proc could bind a parent proc mount. An intermediate
--mount-proc change fixedPIDnumbers but FAILED a stronger outside-file test
through/proc/*/root. That change was rejected. Final current fix movesPIDnamespace
creation toBubblewrap --unshare-pid; enclosingunshare still ownsuser/net/mount
namespaces andfork/kill-child. Actual local+private-worker test checks matching
PIDs and cannot reach an outside test marker through any visible process root.
PASSafter-003. Firmwareconfinement also checks PID match beforecodeexecution.
Serial diagnostics stopreading a childPID once reaped and do not signal it.

Fullfinding in`work/simulator/reviews/proc-namespace-127.md`, linked fromREADME.
Allprior isolation claims remain incomplete; oldevidence unchanged. This also
applies to ongoing125local and126SSH, which use the oldlauncher. They remain
bounded inside disconnectedlocalVMs but cannot grantisolation acceptance.
Source127 has NOT been frozen/builtyet. Fullchecks ACTIVE46536; inspect new
proc-namespace-{clippy,workspace-tests}-127 logs. Nextbuild all6helpers andCLI+
coordinatorbins, realSSHregression, then prepare/runnewfirmware withcorrected
launcher andguard. Do not mistake newtest pass for fullfirmware acceptance.

### 22:42 UTC: correction127 checked, nested proc failure, preparation follow-up128

Source127 frozen:345files, library35e39d3a227e0e6acdce07e76577ce9fd1f579276cf4ffedb39ad2a14be9b46d/32324538.
All412Rusttests across94binaries PASS; strictClippy PASS; guide428links PASS.
All6helpers,CLI,coordinatorbins built inssh-target-127. ActualSSHfixture
network-vm/ssh-isolation-127 FAILED: nestedruntime cannot mountprivateproc
under outerroleBubblewrap readonlyprocchildmounts. Kernel logsVFS Mount too
revealing. This is an actual environment refusal, not a test-program pass.

Originalfirmware local-pipe-127 ran correctedruntime/guard:163409ms,exit0,
complete trace. OriginalSPL/U-Boot/Linux/nativebosminerexecuted. NO poolsession
completed:libslirp reports slirp_socketpair():Operation not permitted. Pipe
forwarding usesprivateAF_INET internally; hostpolicy deniesit. No miningpass.
Preparedprocess127 remainsstored; do not rerununchangedhostpipepath. The network
namespace also hasloopbackdown under currentunshare policy, which needs an
explicit private-loopback policy for this mode inside the authorizedVM.

Localnested125 FAILED:1200065ms timed-out,fulltrace, no mining. Extracted with
125helper into board-local-125/board-result. SSH126 stillactive46468. Both use
oldisolation; neither cancloseisolation regardlessofotherresults.

Reviewfound3moreoldnamespace launchers:firmwareprepare, ELFinspector, and
frozen cgminertoolchain replay. AllnowdelegatePIDnamespace toBubblewrap.
The ELFinspector additionallyretainsunshare--kill-child. Pending128validation.
RemoteVMrolebootstrap nowcreatesa freshprivateproc afterBubblewrap hascreated
thePIDnamespace andpivoteditsroot. This is trusted infrastructure in the
disconnectedVM, notfirmware. The actualworker stilldrops toUID1000 andevery
acquiredprogram uses the strictinner runtime/protectedproc. This avoids the
parentreadonlychildmount restriction without sharingtheparentprocessview.
Newroleassertions checkPIDmatch and noorchestrator paths throughvisibleproc
roots. Failedfixture nowretainsboundedclientstdout toexposeexactfailedresult.
ActualVMSSHtest mustpass beforeacceptingthischange. Noexternalhostclaim.

Source128 fullchecks/freeze/build ACTIVE17951. It builds all6helpers plus
freeze_cgminer_toolchain andcgminer-lab;CLI+coordinatorbins afterwards. Next:
actualSSH128; actualcgminer preparation/replay withcorrectedPIDguard; add
explicitprivate-loopback policy +actualsandboxTCPtest. No newfirmware timer
increase. No agentsavailable: all3remainaccount-limitblocked untilOct25.

### 22:56 UTC: private loopback129 and actual S9 SSH baseline run

SSH128 PASS. Report network-vm/ssh-isolation-128/remote-report.json pins
component b9173d5c79c95a015dfce1e28a4bbe4c45fde3b5d9e4008efc12eecc2034c59e/9854.
Frozen source128 full412tests/94binaries+strictClippy pass. Both cgminer
preparation paths now pass with the corrected PID guard. Frozen replay binary
88602d125e46310d0e61f64622d50be25df285085c2fe09c63070d852c64a90a/515224,
installed UBSan binary0b18f7f5aa273df38e204690883cf948b6d23be0e294085719b87b9ff6358326/3077656.
The source fixture mining-proc-128b computed7799hashes,2device results,2work
generations;fresh-workpass. Both separate Core nodes accepted its freshblocks
and preservedheight1 afterrestart. Evidence bitcoin-core/cgminer-proc128-{0,1}.
Original-board/exacttarget gates remainopen. First run128 failed becauseadapter
binary wasnotbuilt; kept. Binaries were thenbuilt fromfrozen128source withthe
absolutevendoroverride, and128b is the actual successfulrun.

Source129 adds explicitProcessSpec.loopback bool. Defaultfalse is omittedon
serialization, preservinglegacyinput identities. True changeslockedrunhash.
Bubblewrap creates a fresh network namespace and enablesonlyitsloopback.
No hostroute/interface/fallback. NewS9flag private-loopback is restrictedto
localpipepool. RealTCPtest executes locallyandthroughSSH, checks onlylo,
no default route, matchingprocPID, randomchallenge roundtrip and distinctlocal
netnamespace. Sameprocess withloopbackdisabled failsatTCPaccept asrequired.
All412tests/94binaries PASS,strictClippy PASS,guide428links PASS. Frozen129 has
346sourcefiles; all6helpers+CLI/coordinatorbins inssh-target-129. ActualSSH129
PASS: network-vm/ssh-loopback-129/remote-report.json,component
3821d7d63de3d5a58abafe183cd1b428a5071977ad0c0afbcb4f7cb7152336f7/9854.
This remainsa protocolfixture, notfirmware/native-dashboard acceptance.

SSH126 FAILED inner1194080ms exit1,complete trace, no poolsession. Extracted
with126helper toboard-ssh-126/board-result. No activeoldlauncher runremains.

NewSSH129 ACTIVE91563: network-vm/board-ssh-existing-129. Selectedworker
run-1a7cb1679cc64ba54628c874973f1ff50cc977f8ce296daec3ca5c8a9d18b4e4.
Prepared worker-prepared-existing-129 ProcessSpec
4633fff754936b938ae5bfed81c1b02cea9414e0e3414824f358b1c7981ac70d/10084.
Correctedruntime/guard, private-loopback,1200s/1190CPU nestedprofile, unchanged
backend078. It restores actualcleanprovisionedSD baselinefromupdatecycle004:
sd-snapshot-0.json, gzipf4fb5a8b6980ac7f6e220f604bc1d6c981e08ed7ccc877d5a956a80c0fadd6b1/154015.
Newderivedconsole-existing-pool-129.sh hash7b8c883abf56e637f2b6fbf468a4742ba6a3fb88ada1bb3a20b39c7fe736bfe9/3033;
provenanceinmatching.json. It retainsnativepersistedpool/network/MACchecks,
omitsupdate-onlysysupgrade.confappend and insteadassertsoriginalconfig hash.
This is explicitly a persistedbaseline, notblank-stateboot. Driverfolder
board-driver-existing-129; logboard-ssh-existing129.log. At22:55 actualarchived
U-BootwasreadingFIT. No miningpass yet. Sameidentitylocalrun stillrequired.

### 22:56 UTC: QEMU replay and build reproducibility investigation

Replayed exactbackend078buildProcessSpec undercorrected128runtime. Firststore
copyfailed policy(maxobject2GiBvsCLI8GiB); newstore128b copiedonlyobjectfiles.
backend-replay-128b PASS187607ms,exit0,fulltrace,UBSan; alloriginalsmokespass.
Binarychanged: add6e8de2196a37b13ff719d0080bbf421bacb6482eb8feae156f7e797e148c2/19879480,
so it is NOT treatedas078identity. Originalsource/core/toolchaininputshashes
andconfigurestdout areidentical. Static symbol-spancomparison found92changed
spans, primarilygeneratedARMdecoderfunctions; allsymbolnamesmatch. Staticread
of archivedscripts/decodetree.py verifies91e068b1993166ba9bf259573f457342c54de70a117f7db42355a3a50fcc0c68/50782,
parent2a40e6705e59fd8807012631bc6b2c0877f10481b12768f78e4f9d7b88376cce.
ItsTopologicalSorter emitsfromanunorderedset. Hypothesis:Pythonhashrandomization
changesgeneratedoperationorder. Twoactualreplays withPYTHONHASHSEED=0 are
ACTIVE27715(backend-seed0-130a),29516(backend-seed0-130c).130b FAILED before
execution becauseitusedthesamestorewriter;130c usesaseparatestore. Do notclaim
reproducibility untilbothfinishedbinaryhashesmatch.

Currentpost129source modifiesonlybuild_qemu_backend recipe: requirefixedseed0,
verifytheactualdecodetreehash,andrecordthebuildsetting. NoCPUcodechange.
Scopedchecks ACTIVE37726; inspect
build-seed-{clippy,tests}130.log. Source130 NOT frozen/builtyet. Afterchecks,
freeze130 andbuildrecipe;actualrecipevalidationstillneeded. No releasegatecloses.

### 23:12 UTC: build replay, console experiment and evidence correction

All three seeded QEMU builds passed and produced the same executable:
8b287152de6a91085203129b590d864b3d026ea3074080d8c0c48103ca6b2e46,
19,888,568 bytes. The third executes the corrected source130 recipe.
See qemu-s9/backend-build-reproducibility-130.json. Source130 is frozen.
The replay package backend-recipe-130d now includes the exact executed spec
under the required spec.json name, plus a declared replay input manifest.
No original archive was changed. Previous firmware results are not reused.

Source131 adds an explicit batch-console experiment. It runs the original
/bin/sh with +i, then requires the actual shell flags to exclude i. The
adaptation is recorded. This tests whether line editing causes the slow
nested serial setup. No performance claim is made yet. All412 workspace
tests across94 binaries and strict Clippy passed. After a final parser fix,
scoped checks passed. Frozen131 and all selected SSH binaries built.
Session77282 now runs native-batch-131 with backend130d and the private pool
VM. It exercises original firmware, mining, the native HTTP dashboard,
corrupt-update rejection and native service restart. The first command
failed before execution because replay packaging lacked spec.json. Log
native-batch131.log is retained; actual run log is native-batch131b.log.

Correction to22:56: the baseline console checks saved pool config and MAC,
but WRITES static network settings and restarts the network. Initial DHCP
readback does not prove a persistence defect. The old cycle004 adaptation
manifest incorrectly describes initial and resume network behavior as the
same. See reviews/network-baseline-131.md. Captured evidence is preserved.
The source generator is corrected after the frozen131 checkpoint.

Active129 runs:91563 SSH,59175 matching local. Both still have zero mining
acceptance evidence. Original native boot and startup logs are live. These
are baseline restores, not blank-state or network-persistence acceptance.
All three review agents still fail with account usage limit untilOct25.

### 23:16 UTC: current actual results and active sessions

Native-batch-131 PASS: exit0,308296ms,complete trace. Actual archived S9 boot,
native services and27 native HTTP observations pass. The damaged signed image
was rejected without install controls. Native stop/start restored fresh work.
Pool sessions0 and4 completed; all7 sessions, including failed V2 probes,
are retained. Audit0 independently checked12792computedhashes,13serialresults,
2freshjobs,clean transition,difficulty change and8submissions. Its2Core nodes
are running (12665,99109). Audit4 runs25662; inspect before Core submission.
No exact target, full native browser, SSH board or release gate closes.

NAND storage against reproducible backend130d PASS under corrected runtime:
nand-storage-proc-131/assessment.json. Actual writes, erase, spare bytes,
restart/readback and unsupported geometry/format cases ran with UBSan.

SSH-existing129 FAILED: timed-out1200078ms,complete trace,zero pool sessions.
Failure evidence extracted with129helper into board-result. Live native boot
and script reached network configuration but not miner startup before limit.
Matching local129 still runs59175. New SSH-batch131 runs24667, selectedworker
run-bcf3aa732695bb2f8f2d7d0d50a013370f51a8ddeb7c0d15be5a9a052579fb88.
It uses source131,batch console,backend130d and the same saved SD baseline.
ProcessSpec6b62e7010eaaad1546d6e4ed2dcd5894dbd5be015bca3de7be27e02889609038/10120.
Its startup has reached original kernel NAND partition discovery. No pass yet.

Source132 adds BM1397 ordered-clock register storage, per-chip readback,
reset/power-loss handling and a literal guest packet regression. No fake
clock status or hash result.14component tests+scopedClippy pass. Original
Bitaxe probe034 executes unchanged firmware and reaches core-register0x3c
(0x80008074), the next visible unsupported command. Exit1,2932ms,full trace.
Evidence bitaxe/original-probe-034/stage-audit-root.json. Source132 frozen346files.
Fullworkspace checks60629 active. README changed after freeze to link actual
S9/HTTP/build evidence; model/source code unchanged since frozen132.

### 23:22 UTC: verification and next runs

All4native-batch131Corechecks PASS. Session0blocks01ff78ae03fb0dc559b9d81997fa26f80ccc0e0e3e06e4b34ee8c555c88a4ac9,
05a2acfa0021301395fbef3dd1c09363008a04148479962a1f18e1aab996e981;
session4blocks091adaf002a058b2d3a5ec63a368f1bf65c443750c587f45301c1908ad626ce8,
0535b91a0221a7ef94b29372e9b9aeb43476bb409fe8082ecafce86b448cd46e.
Both independent audits check12792totalrunhashes,13serialresults; these totals
are shared run totals, not additive per-session counts. Each selected session
has2freshjobs,cleantransition,difficultychange. Coreevidence under
bitcoin-core/s9-batch131-session{0,4}-{0,1}. All4height0to1persistafterrestart.
The5noncompleting pool connections are retained; do not classify their protocol
without their own trace check. This corrects the loose prior V2 label.

Fullsource132 Clippy PASS,413tests/94binaries PASS,fmtPASS,guide434linksPASS.
README and proc review now link current evidence. Source132code is frozen;
README updates afterfreeze are documentation-only and link-checked.

Actual fullnativecycle ACTIVE95916: qemu-s9/automated-update-cycle-proc-132.
Uses source132helperbuild,backend130d,pool003,actual signedupdate and captured
Firefox payload native-driver-024. All8needed helpers rebuilt from frozen132
source. It will require4nativeboots, independentdiskchecks,8Coreacceptances,
actualsignedinstallation andbrowser/configrestore. No pass yet.

Local131 driver prepared as network-vm/board-driver-local-batch-131, but its
VM has NOT started yet. SSH131 active24667. Old matchinglocal129 remains59175.
Do not confuse preparation with execution. Prepared console-startup-diagnostic-
133.sh/.json adds realprocess/service observations during the unchanged bounded
miner-start wait. NOT executed. Use only if131 fails; do not retry unchanged.
No NAND boot persistence implementation was made yet; the existing component
storage test passes but boot still resets NAND to erased memory.

### 23:29 UTC: paired NAND storage implementation

Source133 is frozen (346 files). Formatting, strict workspace Clippy and all
413 tests across94 binaries pass. The boot runner and SD inspector now share
one bounded overlay codec for SD and reference NAND. Existing SD schema and
input names remain valid. NAND uses a distinct schema, marker, filenames and
131072*(2048+64)-byte geometry. Cross-kind and missing-spare inputs fail.
A restored pair must reference the same actual native restart observation.
The generated NAND backing has all data/spare bytes FF. It is separate from
archived firmware. Only its private qcow2 overlay is writable. The existing
compiled NAND backend handles actual program/erase operations. No CPU or
model binary changed. The CLI requires save-sd and native reboot/update for
nand-storage; nand-state also requires the paired sd-state. The same report
code retains both overlays locally and through the worker.

Actual NAND boot test ACTIVE31619: qemu-s9/nand-boot-write-133. Source133
helpers, reproducible backend130d, full original S9 boot and private pool VM.
It configures fresh mining through the actual native miner. Then the original
mtd tool writes44 known test bytes to the run-owned miner_cfg partition, reads
them back, and requires native restart plus paired disk capture. The explicit
fixture write is isolated from all physical devices. Its console derivative
is console-nand-write-133.sh, SHA256
a81e365ade412cfb65b0a282eefd3065bc57460c34c20bf7012497c0241a848a/3633.
First erased-page oracle: SHA256
d0ff1b294b5288d1ae1421eadf5b2d38a8752b76d472ff30bed9028e25b1c5b8 for2048FFbytes.
The initial development script mistakenly requested42 read bytes; wc measured44,
and that mistake was fixed before execution. No test assertion was removed.
Next: verify actual write/restart, restore BOTH states, read same data and erase,
then another native restart/readback. Run independent mining/Core checks for
fresh sessions and SD inspection. Do not claim persistence from capture alone.

Matching local129 FAILED: timed-out1200072ms, full trace. Extracted failure.
Matching local-batch131 is now ACTIVE52806. SSH-batch131 remains24667, waiting
for actual miner startup. Full update/browser cycle132 remains95916; its
provisioning boot, disk inspection and2Core blocks pass; run0 is active.

Prepared a different clock diagnostic134 using the frozen131 helpers, not
source133: worker-prepared-icount-134. QEMU fixed instruction clock shift6,
unchanged virtual bus timer periods and wall-clock hashing limiter. Adds the
prepared native process/service wait observations. This tests a possible
nested instruction-translation/timer starvation problem. It is an uncalibrated
clock policy, not a measured CPU or timing pass. Prepare session50973; no VM
execution yet. Wait for the131 failure/result before starting this changed
experiment. Review previous icount/TTC history before changing model timers.

Bitaxe core-control research remains unresolved. Archived ESP/Gekko agree on
0x3c=0x80008074, but bit meanings/core state need evidence. The public core
control SVG cannot be read by the web provider; commit API access failed.
Provider failure bytes are saved as bitaxe/core-register-reference-errors-132.json.
No unsupported core command has been converted into a fake success.

### 23:39 UTC: NAND write passes; stronger restore validation runs

NAND write133 passed: exit0,236957ms, complete trace. The unchanged native
mtd tool wrote44 bytes to miner_cfg and dd/cmp verified exact readback. Native
restart exited0. Paired snapshots exist. NAND backing is
fbee2e0f41769c042340dff3211f37f3933c097684bce976a69222767d10656b/276824064.
NAND qcow2 output is e5eebfc5bd3963c79e30b462adb8519b1f27e1de9e14b285b6bdd91038afafe0/19922944;
gzip a9c83f75bc6aa734a7e7af98d77771607234b3af226ab70ccab8af012a2ea0b6/24956.
Its independent audit passes36771 total hashes,45 serial results,43 firmware
submissions,2 fresh jobs, clean transition and difficulty change. Both Core
nodes accepted the fresh blocks and retained height1 after restart. See
bitcoin-core/s9-nand133-session0-{0,1}. Strict independent SD check passed
(e2fsck-f-n exit0); no journal replay or inspection modification was needed.

Review found that equal shutdown text alone does not identify a parent run.
Source134 added comparison with the actual parent result, process spec and
both retained exports. Source135 also validates the recorded restart's full
serial bytes against its digest. Its unit regression rejects recomputed
manifest/sidecar hashes when content is absent from actual output, and rejects
truncated parent traces. Full134 workspace checks passed413 tests/94 binaries
and strict Clippy. The final135 change passed all affected example tests,
strict scoped Clippy and formatting. Source135 is frozen346 files; seven
selected helper binaries built. Source134 remains a distinct intermediate
checkpoint; no firmware was run with its restore helper.

Restore/erase test ACTIVE27470: qemu-s9/nand-boot-read-135. It uses source135,
backend130d and BOTH actual133 snapshots. The new parent proof verified
execution ff1e4af67158e16eca8ef9d699a1ee3f42e9b326d523ec6893fa631e9c085e88/31993567
and spec288b3041d7100f2f523bff19f8ba2dda2199d5ee410f9d5cc3156632cd545dc4/11734.
Console-nand-read-erase-134.sh hash2b2f60cec216c36438cc6488e728ac2ede1618b727c0d013c0e0075ec7d3be6d/3791
requires saved data, native erase, erased-page readback and another native
restart. It retains the original empty uEnv and explicitly sets network
configuration; it does not use the unrelated stable-MAC assumption from129.
Provenance JSON files accompany both scripts. Next: third boot to verify
restored erased data; independent mining/Core and SD checks for new runs.

SSH-batch131 FAILED timed-out1200073ms, complete trace, zero sessions. Actual
failure retained under network-vm/board-ssh-batch-131/board-result. Matching
local-batch131 remains ACTIVE52806. The changed instruction-clock diagnostic
SSH134 is ACTIVE11135, frozen131 helpers, selected worker
run-02e3c6e0da8b76006fcd7a8f817a0ce9b2762f41fa777f013ee0d5a78ae82c89.
Its ProcessSpec is442a301d84ce40311a00fa88c2686f19d73969616e18667eb5dbc2fb91c2d460/10170.
It has reached the original kernel. No remote mining pass yet.

Full signed update/browser cycle132 ACTIVE95916. Provisioning and phase0
(signed install, native reboot, configuration write, disk check, fresh mining,
2 Core blocks per boot) pass. Phase1 is running the native browser and saved
configuration readback. Still needs phase2 restoration/restart evidence.
No exact physical target, full matrix or release gate has passed.

## 23:49 UTC — complete local update/browser cycle; NAND export defect

Cycle132 finished exit0. assessment.json passed=true. Provision plus all three
phases passed, including real Firefox login/live HTTP/WebSocket checks, actual
signed installation, changed setting after restart, restoration and final
restart readback. All eight separate Core checks accepted actual fresh firmware
blocks, height0 to1, persistent after node restart. Source132/backend130d;
corrected namespace guard. The update disk needed journal replay on the separate
inspection copy; phases1/2 passed strict raw checks. Original snapshots unchanged.
No exact-board, recovery or remote publication gate is inferred. README links it.

NAND135 FAILED overall exit1 after140012ms, full trace. Actual saved44 bytes
read back correctly after restart; native erase/readback passed; native restart
and emulator exit0 observed. Export then failed: raw overlay exceeded32MiB.
No snapshots survived. The30-second console also ended before the230-second
pool window, so dropping the support VM lost the active authorized session.
Six non-completing sessions were retained. This run does not pass mining or the
full storage cycle. Keep its failure artifacts unchanged.

Source136 parent-result hash/read integrity fix passed affected checks and seven
helpers built. No firmware execution used136. Source137 now compacts NAND with
isolated qemu-img convert over the same erased backing, compares all logical
bytes with qemu-img compare, and checks source/backing hashes unchanged. The
original32MiB export,24MiB compressed and64MiB sandbox file limits stay in place.
It records both source and export identities and exact tool. SD layout stays
unchanged. A native storage checkpoint now waits for all independent pool
sessions and the first authorized session's completed suite, within existing
budgets. A later success cannot hide an earlier authorized failure. Regression
checks exercise this condition and incomplete/ambiguous collection rejection.
First Clippy run caught duplicate module inclusion; corrected to shared import,
not suppressed. Current137 full checks/build pipeline ACTIVE42800. Freeze and
helpers occur only after checks pass. Next repeat135 with137 and fresh pool seed,
then third boot to check erased data before any write.

Local-batch131 failed timed-out1200054ms with full trace. Extracted failure at
network-vm/board-local-batch-131/board-result. SSH instruction-clock134 still
runs; native services now show some progress. No remote mining pass. All three
review agents remain unavailable due account usage limit untilOct25; unchanged
quota failure was not retried. Independent review gate remains open.

## 23:54 UTC — revised NAND execution active

Source137 full checks passed417 tests/94 binaries, strict Clippy, format.
Freeze137 and seven release helpers built. Repeat restore/erase ACTIVE23973:
qemu-s9/nand-boot-read-137, exact133 SD+NAND parents, unchanged console134,
backend130d, new random pool input. Actual native saved-data readback and erase
already passed; the new runner is keeping the miner alive for the pool window.
No complete pass is claimed yet.

Source138 additionally preserves both capture attempts and finishes the pool
when one export fails. Each capture error is retained with storage kind/stage;
any error still fails the run. Scoped boot tests, strict Clippy and formatting
passed;138 frozen and seven helpers built. The third boot will use138 after
137 supplies a valid pair. Prepared read-only NAND phase script:
qemu-s9/console-nand-erased-read-137.sh,
e1777ece4a95162313c076ce73aa7b937390583fe3ebee33823f4898249d4130/3584.
It reads the erased page before any NAND mutation in that phase. Provenance JSON
is adjacent. Preparation is not execution. Source136 was superseded before any
firmware run; retain it as a separate checked intermediate.

## 00:01 UTC 22 October — next bounded SSH test and NAND geometry bound

SSH instruction-clock134 FAILED timed-out1200071ms, full trace, zero pool
sessions. Actual failure extracted to network-vm/board-ssh-icount-134/board-result.
Cleanup acknowledged selected run02e3c6e0da8b76006fcd7a8f817a0ce9b2762f41fa777f013ee0d5a78ae82c89;
published inputs preserved. The full process observations show steady startup:
S11sysctl at guest191.78s, S12rpcd225.24s, S19firewall256.56s,
S50dropbear306.46s, native uhttpd332.56s, S99bos_autoupgrade355.94s.
This is slow nested execution, not proof of a boot deadlock or missing keys.
The1800-second host profile in source140 is distinct from the retained1200
profile. It keeps the same guest instruction clock and bus timing, gives1500s
boot allowance plus the unchanged230s pool window,1790s CPU limit, and bounded
outer client/server/VM observation windows2160/2280/2400s. Memory/file limits
and all firmware/mining assertions stay unchanged. The old1200 profile remains
supported. Full checks/freeze140 ACTIVE34577. Build command initially named
nonexistent packages md-sim-cli/md-sim-worker; corrected actual build uses
md-sim,md-sim-coordinator,md-sim-firmware (build-remote140b.log).

NAND137 FAILED exit1,247530ms, full trace. Actual saved-data read, erase and
native restart pass. qemu-img conversion and complete logical comparison
succeeded, but converted output still exceeded the shared32MiB SD bound.
The new pool wait retained all10 sessions, including actual first authorized
session0:completed=true,99 submissions. Independent audit correctly refuses
an overall failed firmware runner; nand137-audit0.log records this rejection.
Do not turn this partial evidence into a mining pass or override that check.

Source139 gives NAND a separate64MiB raw export limit for its264MiB data+spare
geometry. SD remains32MiB; the process per-file limit stays64MiB, gzip24MiB.
It records conversion identities before bounds checks. Tests reject larger
NAND snapshots and preserve cross-kind/geometry rejection. Source138 evidence
retention remains included. Scoped checks/freeze139/seven helpers pass. Actual
repeat ACTIVE76613: qemu-s9/nand-boot-read-139, same133 parent pair and console134,
new pool seed, backend130d. No third boot can start until a valid pair is saved.
Sources138 and136 were superseded without firmware execution.

Additional Bitaxe research found GPTechinno/bm1397-protocol and bm13xx-rs;
reference directory lists actual register.rs/core_register.rs files. Their
source bodies and immutable commits could not be fetched by the provider.
Saved provider discovery/errors in bitaxe/bm1397-{library-discovery,control-source,control-github}-140.json.
The newer Mujina reference discusses BM1362/66/68/70; do not assume its core
mailbox fields prove BM1397 behavior. No unsupported command was bypassed.

## 00:14 UTC 22 October — three native NAND phases pass; compact layout active

NAND read/erase139 PASS exit0,247750ms,full trace. Its audit verifies116937
computed hashes,112 serial results,106 submissions,two fresh jobs,clean
transition,difficulty change. Both Core nodes s9-nand139-session0-{0,1} accepted
and persisted the actual blocks. Strict SD inspection exit0, no journal replay
or modification; partition3bcbb08715cc0d6603c9fa536321f3ca1d6f142a2eb4c13cf4ecb513637e1c99/67108864.
NAND exportdb6a39a0f9688acf9c41e1d185bcfcad0baaa0fd418e9780bb5f23d138a60ee6/39649280;
gzip0750e94c8486b2fa9289bcaf7049331728299228b7b178545b1e4d638b9c54ac/48339.
Source overlay1fb1e53a504d315a0362b362e2a2cd59d4281b45d0a3efef14300d89407b096e
has same size; isolated qemu-img compare verified all logical bytes unchanged.

Third boot nand-boot-erased-139 PASS exit0,248120ms,full trace. It restored
both139 snapshots and verified the erased page before any NAND mutation in
its test script. Audit109025 hashes,115 serial results,111 submissions,two
fresh jobs. Both Core nodes s9-nand-erased139-session0-{0,1} accepted/persisted;
strict SD check passed. NAND rawd90cee7f563a68f90c9132da27386b7e4e9472c593baf16f6c9f689f0394c313/57933824;
gzipcb89789aee51ee99c39cc403ab889803b3e85fd79f8f8a16c14a52fbf25fd18b/70034.
Together with initial133, all three native phases and six Core checks PASS.
Developer review reviews/nand-persistence-139.md records exact scope/limits;
README links it. This is not full NAND, exact-board, independent review or release acceptance.

Allocation investigation: native UBI attaches the144MiB antminer_rootfs region,
creates internal volume metadata and writes small headers. The64KiB qcow2
clusters amplify such writes. Source141 creates NAND with4KiB clusters and
uses qemu-img convert -c -o cluster_size=4096,compression_type=zlib both before
restored execution and after capture. Full qemu-img compare and unchanged
source/backing hashes are required. Only the decoded private copy is replaced;
input gzip and backing remain read-only. All existing resource bounds remain.
Exact conversion argv/tool/result identities are retained. Scoped checks and
freeze141/seven helpers pass. Full fmt/Clippy/tests/guide pipeline ACTIVE47190.

Actual compact restore ACTIVE94115: qemu-s9/nand-boot-compact-141, from actual
third139 pair. Conversion succeeded:57,933,824-byte source to446,464 bytes,
SHA7a765b5955dfb0e6b443bb3c2c9b6e51563ee2f64d6ec81c7fe21ff83f84ddad,
4096-byte clusters, full logical compare exit0. Native execution is ongoing.
New cold write ACTIVE66200: qemu-s9/nand-cycle-write-141. This tests the changed
initial creation branch using original133 native write script, new fresh pool
seed, same backend130d. Next restore this pair for read/erase, then erased
readback using141 throughout, with audits/2Core/SD checks for every boot.
Arbitrary full-media writes remain outside this bounded test and need their
own output/space contract; do not claim they passed.

Long remote/local tests use frozen140, all440 workspace tests/94 binaries and
strict Clippy passed. Correct build build-remote140b.log passed. Both use the
same prepared ProcessSpec30b0647507ed2745a40feaeaed7226cc2bcb2676a7202296858948897d991d8b/10175.
SSH ACTIVE2125: network-vm/board-ssh-long-140; driver board-driver-ssh-long-140.
Selected run316bd02a0c5021f3691a69c2888b6d3046f38bd99954306ed5d5a4169e33fd1f.
Local ACTIVE68538: network-vm/board-local-long-140; driver board-driver-local-long-140.
Prepared input qemu-s9/worker-prepared-long-140. Each outer VM is disconnected;
this is not an external physical host. No remote mining pass yet.

## 00:30 UTC 22 October — final NAND141 cycle passes; automated142 running

All three source141 boots PASS with full traces and native restart:
- nand-cycle-write-141:248474ms,66330 hashes,76 serial results,69 submissions.
  NAND rawe56b2fd360feaafd1afa1ce35e2791fd1705b8437d7ef32184ee4a92830366be/155648;
  gzipaf05a0aed91ed2bc110af925d180c73d4ac73c86b1801b4bf141ca1313820729/13046.
- nand-cycle-read-141:249275ms,93240 hashes,65 serial results,63 submissions.
  NAND rawaf7beffb54f97d8f1a64eabe63885f7942fd22b8f10692d5cbc44b2257091925/282624;
  gzip3bae555a35dab4e17c6c7e46d66f2c10dd10299a35090ad716954d4052531ce6/24653.
- nand-cycle-erased-141:250304ms,112008 hashes,125 serial results,122 submissions.
  NAND rawcbdd7879a3db5564366b6ae24ff121821f7ca14b194639e23da5e0af41ad8c93/405504;
  gzip0ca0ead6a1ce1e50488e60f009de15e507ca1e3404bfdc91732dc5267a50febc/35816.
All have two fresh jobs, clean transitions, difficulty changes and strict clean
SD checks. All six independent Core nodes s9-nand-{write,read,erased}141-session0-{0,1}
accepted from height0 to1 and persisted after restart. Compact migration141
also passed audit95409 hashes,70 serial results,67 submissions,two fresh jobs,
strict SD inspection and both s9-nand-compact141-session0-{0,1} Core nodes.
The three-phase predecessor and new141 use the44-byte fixture;
this is not full-media, physical-board or power-loss acceptance.

Source142 adds s9_nand_cycle, a complete Rust test driver for three actual
archived boots, fresh4160-byte storage data spanning three NAND pages,
write/read-after-restart/erase/erased-read-after-restart, verified paired
snapshots, strict SD checks, fresh mining audits and six independent Core nodes.
It freezes programs, scripts, backing manifests, expected data, and every
completed parent input consumed by the next boot. It checks actual SD and NAND
input identities and phase markers; all errors remain failures. A new outer
set-e wrapper prevents an earlier failed console subshell from being hidden by
a later successful storage subshell. This is native test orchestration, not a
new firmware miner or device model.

Shared cycle evidence functions moved from s9_dashboard_cycle into
support/s9_cycle_evidence.rs. Shared saved-pool console generation is in
support/s9_cycle_console.rs. The unchanged console-private-pool-014.sh bytes
are now in simulator/scenarios/s9-private-pool.sh with parent/hash provenance.
No downloaded input was changed. The shell is required to use the original
guest's native tools; the new orchestration and verification are Rust.
Scoped strict Clippy and8 tests PASS. Release142 ten helpers built. Frozen
core-checkpoint-142 retains352 source files. Full Rust checks ACTIVE31565.

Automated NAND ACTIVE56110: qemu-s9/automated-nand-cycle-142. Driver command:
work/simulator/ssh-target-142/release/examples/s9_nand_cycle work/simulator/original-zynq/sd-bundle-006 work/simulator/qemu-s9/backend-recipe-130d /tmp/md-sim-qemu-pool-003-ubsan work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin work/simulator/qemu-s9/automated-nand-cycle-142
Automated signed update/browser regression ACTIVE23205:
work/simulator/ssh-target-142/release/examples/s9_dashboard_cycle work/simulator/original-zynq/sd-bundle-006 work/simulator/qemu-s9/backend-recipe-130d /tmp/md-sim-qemu-pool-003-ubsan work/simulator/qemu-s9/automated-update-cycle-142 signed-update=work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin browser-payload=work/simulator/network-vm/native-driver-024/payload.json
Logs are checks-2026-10-21-001/automated-{nand,update}142.log.
Both use backend130d and new seeds. Do not count either as passed before its
assessment and all independent checks exist. They remain adapted reference
cycles, not exact hardware acceptance.

Long local/SSH140 still active. At00:28UTC SSH native bosminer is running
(pid2086), after roughly1370s outer time; no mining pass yet. Actualselected
run remains316bd02a0c5021f3691a69c2888b6d3046f38bd99954306ed5d5a4169e33fd1f.
All three review agents still fail the same account usage limit untilOct25.
Do not retry unchanged; independent review remains open.

## 00:35 UTC 22 October — remote transport140 passes; mining140 fails; fix143 active

Both long140 executions finished exit0 with full traces, actual archived SPL,
U-Boot/Linux and native bosminer startup. SSH1367385ms, local1409931ms. Both
actual local/SSH transport reports pass, including result retrieval and cleanup.
They do NOT pass mining: both retained zero pool sessions. Both independent
audits fail with `no computed nonce traces; no audit pass`. The TLS fixture
in these tests is separate and does not test the selected board dashboard.
Reports: network-vm/board-{ssh,local}-long-140/{remote-report.json,board-result/}.
The SSH service is the actual selectedrun316bd02a0c5021f3691a69c2888b6d3046f38bd99954306ed5d5a4169e33fd1f.
No Core attempt is appropriate for these failed mining audits.

The actual logs show a native bosminer PID but API connection refusals during
the30 guest samples. The console finished, then the runner killed its owned
emulator before the declared pool window. Source143 corrects this direct pipe
pool path: keep the live guest and drain its serial output until the collector
finishes at the existing deadline; require all sessions retained and require
the first actual authorized V1 session to pass. Empty collections fail. This
shares the same completion validator as the VM pool. No work deadline, guest
clock, hash cap or outer budget was increased. finish() can join only once;
Drop still stops owned work after an error. Regression tests check that waiting
does not stop an active producer and that an empty collection fails. Existing
failure-selection and incomplete-collection checks remain strict.

All446 tests/95 test binaries, strictClippy andfmt PASS for143. Release143 remote
binaries/nine examples built; core-checkpoint-143 retains352 sources. Prepared
qemu-s9/worker-prepared-window-143 has ProcessSpec1185da88a7433d31b6ebd0c4a3f231fda326f69ea4978f35cde448ba0e41bda4/10175.
It uses the same baseline004, startupdiagnostic133, backend130d, nested-tcg-1800,
private-loopback, cap1000, icount6 and monitor adaptation as140; freshpoolseed.
Both new executions use that exact prepared ProcessSpec:
SSH ACTIVE64809 network-vm/board-ssh-window-143; driver board-driver-ssh-window-143.
Local ACTIVE55619 network-vm/board-local-window-143; driver board-driver-local-window-143.
Logs checks-2026-10-21-001/board-{ssh,local}-window143.log and matching report logs.
Prepare with remote_namespace_check prepare-board[ -local] PREPARED BUILD/release
work/simulator/network-vm/scenarios/namespaces-001.json NEW_DRIVER, then
isolated_network_vm NEW_RUN NEW_DRIVER/scenario.json, then remote_namespace_check
report NEW_RUN. Use source143 helpers for extraction if the test fails.
These are disconnected local VMs, not an external physical host.

Source142 full445tests/95binaries,Clippy,fmt passed. Automated NAND142 first
write phase passed actual boot/native4160-byte write, SD inspection, independent
mining and both Core checks. Phase1 is now active. Automated signedupdate142
is also still active. These processes use their immutable142 binaries while
current143 changes the separate direct pipe-pool lifetime path.

## 00:50 UTC 22 October — NAND142 passes; dashboard bridge144 local passes, SSH startup fails

Automated NAND142 PASS assessment.json: all3 original boots, native fresh4160-byte
write, saved-data read/erase, erased-data restart readback, verified paired
snapshots, strict SD inspections and6 Core blocks accepted/persisted. Audits:
run0:71400 hashes,79 serial results,72 submissions;
run1:95991 hashes,92 serial results,88 submissions;
run2:92249 hashes,86 serial results,82 submissions.
Each has two fresh jobs, clean transition and difficulty change. Full-media,
physical-board and power-loss gates remain open. Signedupdate142 still active;
4 Core checks retained so far, first phase complete, browser phase running.

Source144 adds an incoming dashboard byte bridge in md-sim-runtime/src/dashboard.rs.
ProcessSpec.dashboard is optional/omitted when None, preserving legacy serialized
identities. It requires private loopback and declared guest/listen ports, source
label, >=512MiB address space and32 process slots. Runtime binds only worker IPv4
loopback and mounts only one private runtime-owned Unix socket into the guest.
No host directory/network namespace/control port is exposed. A secret runtime
handshake routes incoming streams to the declared private loopback port. Fixed
bounds:8 connections,64MiB per connection,256MiB per run,16KiB directional buffers,
60s idle; cancellation checks100ms;256KiB thread stacks. The guest needs the
explicit trusted Agent adapter. Firmware integration is not wired yet.
New execute_dashboard_recorded saves a live binding receipt bound to canonical
ProcessSpec; local executor/worker/CLI retain it. Worker Status returns it for
active selected runs; result records actual byte/count/lifecycle data. Cleanup
owns the receipt. Existing process literals now explicitly setdashboard:None.
No default process receives a socket or listener. Server/agent threads are owned
and joined. Empty SSH readiness probes are recorded separately and do not open
an upstream native connection.

Source144 full strict Clippy PASS. Release144 bins+remote_namespace_check,
remote_transport_check,isolated_network_vm built. core-checkpoint-144 captures354
sources. Fulltests144 failed compilation on one compact ProcessSpec initializer
in examples/isolated_network_vm/payload.rs:434 missingdashboard:None; fix next.
Earlier scoped cargo check missed cfg(test). No test failure was hidden.
Actual network-vm/dashboard-stream-144 failed after34.8s: local actual isolated
stream test PASSED (65568 request bytes,96 response bytes, early response before
request EOF, half-close, fresh guest challenge, closed endpoint). SSH worker test
failed `worker dashboard binding absent`; statuses showfailed with zero logs.
No remote/native dashboard pass. Parsed failure events retained in
network-vm/dashboard-stream-144/ssh-failure.json. Need retrieve failed worker
result immediately instead of waiting5s; likely private VM /tmp mode0755 denies
workeruid1000 tempfile creation, but that cause needs confirmation. The fixture's
local runner isroot and worked. The plain bwrap client/server setup uses
--tmpfs /tmp without1777 permissions. Review this and retain actualfailure.

Follow-up review also found Agent UnixStream::connect can block if its pending
Unix backlog fills. Replace with a nonblocking fixed-path connection before
claiming cancellation behavior. Add input/budget/stop tests, then repeat actual
local+SSH stream test. Existing SshTunnel creates an empty readiness connection;
the bridge deliberately waits for first client data before opening upstream.
New fixture is transport-only and does not satisfy native firmware dashboards.
Native S9 hook/hostfwd and selected-run browser publication remain unimplemented.

Long actual board143 still active: SSH selectedrunfc80faf2166cfbc2ebf3e2df12fb25bc92b9c027b1670f542a3b963618968ec2;
localexecution09a8fa53729d407be74c5e1e942be6e566c43ded58624ef1019bc031d2540372.
At00:45UTC about640s elapsed; no mining outcome yet. Keep frozen143 helpers for
its independent auditing. Current changes use separate144 files/builds.

## 01:12 UTC 22 October — bridge146 passes; both long board143 mining runs fail

Source146 full449 Rust tests/95 binaries, strictClippy andfmt PASS in checks146
logs. core-checkpoint-146 records354 sources. dashboard-stream-145 retained a
real worker startup failure: Permission denied creating /tmp/md-sim-dashboard-*
inside the worker VM. Private VM tmpfs was0755. Source146 sets that VM /tmp
mode01777; runtime still creates private0700 directories and0600 sockets.
The Unix agent now connects nonblocking before entering a wait. The new actual
network-vm/dashboard-stream-146 local and SSH selected-process tests both PASS:
65568 fresh request bytes,96 response bytes,65664 forwarded bytes; early response
before request EOF, preserved half-close, fresh guest challenge and endpoint
closure. SSH uses the selected worker run79 repeated32 times, with canonical
ProcessSpec binding and client tunnel127.0.0.1:18089. Separate client/worker
namespaces are inside one disconnected local Linux VM, not an external host.
These are transport fixtures, not native dashboard or firmware acceptance.
Source146 also adds dashboard-forward to s9_boot_bundle with private-loopback:
only QEMU native HTTP guest80 via private127.0.0.1:18080 and the run-owned bridge.
This actual native hook has not yet been exercised. Metadata safety, cancellation
and explicit report assertions are the next review changes.

Automated signedupdate/browser142 completed PASS:4 actual archived boots and8
independent Core nodes accept and persist firmware-produced blocks, plus native
Firefox login/assets/telemetry/WebSocket/configuration/restoration/restart.
assessment.json is qemu-s9/automated-update-cycle-142/assessment.json. Automated
NAND142 remainsPASS with3 boots/6 Core checks. Both are adapted reference paths.

Both board-window143 runs FAILED and were extracted with source143's
remote_namespace_check extract-board-failure. SSH process exit1/1733763ms;
local exit1/1733706ms. First authorized session timed out after its declared230s
window without a submission. SSH backend shows real ASIC packets and dummy work
at guest663.8s, plus calculated hash events; this is not fresh pool work evidence.
Pool retained configure/subscribe/authorize and notify but zero submissions,
completed=false, connection_end=work-deadline. Complete collection1session, no
lost reports. Driver fails first authorized mining session; source143 independent
regtest audit rejects the failed runner. No Core test or mining pass is claimed.
No deadlines or assertions have been weakened. Both outputs and exact failure
logs remain under network-vm/board-{ssh,local}-window-143/board-result.

All3 helper agents remain errored due account usage limit until25October08:48AM.
Independent agent review remains incomplete. No exact physical target passes;
full matrix completion and original-board gate remain open.

## Recorded01:24 UTC22October — bridge147 cancellation passes; selected native HTTP148 in development

Source147 frozen355files; full450tests/95binariesPASS, strictClippy/fmtPASS.
Release147 build initially used an incorrect package name md-sim-cli and failed
before compilation; corrected command uses -p md-sim. Build correctedPASS.
Metadata receipt reads now use O_NOFOLLOW/O_NONBLOCK, require regular files with
one link, enforce size before and during read, and verify canonical ProcessSpec,
source, IPv4loopback, ports, fixed resource limits and no readiness claim.
Regression checks reject altered endpoint/identity/limits, symlinks, hardlinks,
directories and oversized files. Shared dashboard_transport_report checks exact
actual guest stream observations, local/SSH same process, binding/final receipt,
and actual open-stream cancellation. The namespace reporter requires these events.

network-vm/dashboard-stream-147/remote-report.json PASS from actual disconnected
VM client/worker namespaces. Local/SSH stream tests stillPASS. A different actual
selected worker run7a repeated32times sent its computed fresh challenge digest,
then stopped with that stream open:229ms, statuscancelled,64bytes forwarded,
stream closed, listener recordedclosed, tunnel stopped, result retrieved/cleaned.
Native firmware gates remainfalse. These checks use frozen147 binaries and source.

Current148 work: selected original-board native HTTP observation via runtime
bridge, validated selected run and (for remote) actual SSH tunnel. Reuses the
existing Boser native login/assets/uptime/configuration read-write-restore probe.
Full dashboard/browser/WebSocket/TLS/persistence gates stay separate. The probe
retains every initial HTTP readiness failure and never treats the bridge TCP
listener as firmware readiness. NOT yet built or executed.

Both143 native board runs reached only dummy ASIC work by1730s. Add explicitly
selected nested-tcg2400 and pool-seconds600 (bounded20..900; default230 unchanged).
Collection ends70s before the declared run budget; session window is capped by
remaining collection time. Guest clocks, bus periods, hash cap, assertions and
archived bytes remain unchanged. New seeds and policy invalidate prior results.
This is measured host execution allowance, not a calibration claim or pass.
New candidate not yet run. Initial148Clippy found duplicate included settings/
update contract modules from the reused probe and pool reporter. Move those two
unchanged shared modules into md-sim-firmware library; users import one copy.
Corrected148Clippy running. Next: build/test/freeze148, run actual local/SSH board
with selected dashboard-forward and explicit new budgets; audit fresh mining
and Core only after the actual execution passes.

## 01:34 UTC22October — guard150 passes; original board150 runs active

Source148 full453tests/95binariesPASS, Clippy/fmtPASS; guide442linksPASS.
Both actual board-dashboard148 attempts FAILED before firmware: the firmware
confinement guard rejected /md-sim-dashboard.sock as a non-regular input. The
actual runtime correctly closed its endpoint. Both failure outputs extracted
with148helpers into board-result. No firmware or dashboard pass. Source149 adds
an exact optional socket mount rule: runtime environment required, sole fixed
mount, read-only, owned Unix socket0600 with one link. Other file/network/proc
checks stay intact. Dashboard supervisor infrastructure faults now fail the
process result instead of silently losing a listener. Source149 failed its
host test because Unix bind is forbidden, and failed its VM stream fixture
because the new full firmware guard was also applied to the generic local
fixture (VMroot rather than nonzero worker). Failures retained; no149pass.

Source150 keeps the positive socket check in actual local/SSH VM tests and runs
the complete firmware guard on the dedicated nonzero SSH cancellation run.
Host tests check invalid environments and reject regular/symlink/directory
substitutes; they do not skip the actual VM socket check. Failed local stream
attempts now retain their full ProcessResult. Full456tests/95binariesPASS;
strictClippy/fmtPASS. network-vm/dashboard-stream-150/remote-report.json PASS,
including actual duplex local/SSH and full-guard worker cancellation. This is
still transport-only. core-checkpoint-150 freezes sources and release150 inputs.

Actual source150 prepared ProcessSpec44895365e6feb39d272626c34a0df6d20ad3349a35e633b6834a117a4ae18c17/10316
at qemu-s9/worker-prepared-dashboard-150. It uses baseline004/backend130d,
startupconsole133, exact native monitor adaptation, private-loopback,
dashboard-forward, icount6, cap1000, explicit budget2400/pool-seconds600.
SSH ACTIVE39962 network-vm/board-ssh-dashboard-150;
local ACTIVE27796 network-vm/board-local-dashboard-150.
Both use the same prepared spec through separate disconnected local VM client/
worker environments. Drivers board-driver-{ssh,local}-dashboard-150/scenario.json.
Check actual outcome, selected HTTP report and independent mining/Core; do not
claim success from startup. Source150 includes a new selected gateway library
in DEVELOPMENT, not wired to CLI or executed. Review found TCP endpoint reuse
can race polling after run exit. Change selected publication to a unique
run-owned Unix upstream before claiming its lifecycle is safe. Generic legacy
gateway remains separately tested. No original-board or exact-target gate closes.

BM1397 exact0.2.0 register index fetch also failed in web provider. Saved
bitaxe/bm1397-index-access-149.json; no unsupported register success invented.

## 01:44 UTC22October — unique upstream151 passes; selected TLS152 test active

Source151 full456tests/95binariesPASS, strictClippy/fmtPASS. Actual
network-vm/dashboard-stream-151 PASS34960ms; reporterPASS. Adds a separate0600
Unix HTTP upstream in the same private0700 runtime directory, while preserving
loopbackTCP access. Both listeners share the eight-handler and byte budgets.
The local Unix test sends fresh data through the actual isolated process,
checks duplex/half-close/challenge and confirms the socket path is gone after
completion. TCP local/SSH and full-guard cancellation also pass. Binding records
the exact unique path. Gateways can use that path, preventing a reused TCP port
from reaching another run after the original runtime endpoint closes.

New md-sim-access selected gateway validates live worker Status, complete
LockedRun canonical identity, ProcessSpec/binding, source and Unix upstream
before/after startup and while serving. Stops on completion, status loss or
metadata changes. CLI serve-selected SPEC LOCKED_RUN SOCKET NEW_DIR added;
CLI JSON reads are bounded/no-follow/regular. This CLI and actual Nginx Unix
composition are NOT yet accepted;152 test is now active. Complete LockedRun
object identity shares its canonical encoding with existing hash(), preserving
old hashes. Access crate imports the coordinator's contract, not a copy.

Source152 adds actual selected TLS fixture: new process7b repeated32times,
full firmware guard, private HTTP service through the runtime Unix upstream,
metadata upload and destination hash checks, SSH launch as worker owner,
mutualTLS request from separate client, independent fresh body digest/guest
challenge, forged guest run-header rejection, no-certificate rejection,
worker stop/result/cleanup and automatic gateway closure. Actual fixture only,
no native firmware gate. Shared reporter now requires those observations.
Server copies only its authorized TLS material into a separate private
worker-owned directory; client and worker remain separate namespaces and do
not share each other's private keys. Generic gateway wording now says
configured CA; it cannot prove that a caller uses a separate CA for each run.
SSH service_command reuses pinned host/auth/tool configuration and permits
only bounded literal arguments; no shell interpolation.

Source152 firstClippy failed a helper path (local_tools is in executor), fixed
and correctedClippyPASS. Release152 built; fulltests152 active. Frozen152
checkpoint retained. Actual new network-vm/dashboard-stream-152 ACTIVE22847; driver
network-vm/dashboard-driver-152 includes /md-sim-access. Output log
checks-2026-10-21-001/dashboard-stream152.log. No TLS pass until report succeeds.

Both actual native board150 runs continue. SSH selectedrun
3c498e4be49ee15b2a1e0e012ba5d3ff90b0fad6fc4be0193316cba3ce529ab6;
local LockedRun d731f08f48fd05c7f3ac6d2a7f7bd945a8fc4ce360d08123753b3b05fb07b400.
At01:44UTC SSH596s, native boot output retained, no final mining/HTTP result.
Use frozen150 for their extraction/audit. They do not include151 Unix/TLS work.

## Recorded after153 test — TLS failures retained; directory permission fix154

Source152 full456tests/95binariesPASS, Clippy/fmtPASS. Its actual selected TLS
fixture FAILED: metadata transferred and destination hashes verified, but the
support launcher used /worker/store instead of the CLI worker's actual
/worker/worker/store. Source153 fixes the path and adds contextual input errors.
Source153 full457tests/95binariesPASS, Clippy/fmtPASS. Its actual selected TLS
fixture FAILED a different check: Unix upstream private-directory validation.
Both failed raw SSH event sets and helper stdout/stderr are retained under
network-vm/dashboard-stream-{152,153}/ssh-failure.json and original VM outputs.
Neither is a selected TLS pass.

Review of pinned vendor/tempfile/src/lib.rs confirms default temp directories
use0777 filtered byumask, not0700. Earlier progress statements that the bridge
parent directory was0700 were wrong. Socket0600 and runtime token isolation
were enforced, but parent directory privacy was not explicit. Source154 now
requests0700 through Builder.permissions AT CREATION. A host filesystem test
checks actual directory type, owner andmode. No gateway assertion was weakened.
154Clippy/build/fulltests active. Freeze154 records exact inputs. Repeat actual
selected TLS fixture next. Original board150 tests continue with their frozen
150 binaries; they do not prove this154permission correction or TLS publication.

## 02:07 UTC22October — selected TLS154 passes; owned deployment155 in test

Source154 full458 Rust tests/95 binaries PASS; strict Clippy and fmt PASS.
Actual dashboard-stream-154 remote-report.json PASS. The separate client
reached selected worker run-7b repeated32times at https://192.0.2.1:9443/ through
its unique Unix upstream. Fresh512-byte request digest and guest challenge
matched actual guest stdout. Missing-client-certificate access was rejected;
the gateway replaced a forged firmware run header. Stopping the selected
worker cancelled the actual process, retained its full trace, retrieved and
cleaned the result, and stopped the TLS listener. This is a protocol fixture,
not native firmware acceptance. The topology is a disconnected local Linux VM
with separate worker/client namespaces, not an external physical host.

Source155 moves hash-checked metadata transfer to the shared coordinator
transfer_object function. RemoteGateway owns an authenticated SSH service
connection, validates the complete selected LockedRun/ProcessSpec publication,
retains transfer and service logs, and requires acknowledged listener cleanup.
The connected helper stops publication when its held stdin closes. The actual
fixture now closes that client input, verifies the worker stayed active, then
reconnects publication to the same selected process and performs the fresh
TLS request and worker-stop checks. Freeze155/build/Clippy complete; full tests
and dashboard-stream-155 (session62033) active. Native board integration of this
reusable transport is next; it has not yet been tested on native firmware.

Both source150 native board runs are still active. At about02:07 local guest
ASIC dummy work had started aroundguest646s, while its pool retained a later
non-Stratum-v1 connection. SSH guest PIC initialization is aroundguest656s.
These are provisional traces, not mining success. Final reports and independent
replay remain required. Frozen150 binaries do not include the corrected154
private directory or155 TLS deployment. All three review agents remain blocked
by their service usage quota until25October08:48; no independent review pass.

## Source155 accepted; source156 native TLS integration ready to execute

Source155 full458tests/95binariesPASS and actual dashboard-stream-155 PASS.
It checks owned SSH input closure, acknowledged publication stop with the
worker still active, publication reconnect to the same process, fresh TLS
body/guest challenge, no-certificate rejection and automatic worker-stop cleanup.
Source156 full458tests/95binariesPASS, Clippy/fmtPASS,444guide linksPASS, and actual
same selected TLS transport fixture dashboard-stream-156 PASS. Freeze156 and
all logs are retained. README now gives the exact tested155 commands.

Source156 board client now starts RemoteGateway for the ACTUAL selected worker
LockedRun/ProcessSpec, after its HTTP probe succeeds. It then tests the native
Boser path over mutual TLS with another fresh private cookie store. Every
response must carry the selected run ID. Login/assets/live uptime/configuration
write/readback/restoration/logout assertions are reused, not copied pages or
an API imitation. The client also rejects no-certificate access and explicitly
stops the owned publication after the probe. Its selected-dashboard schema2
retains TLS publication, native response evidence, transfer/service logs and
cleanup. This native composition has not yet run; source150 active board tests
remain frozen HTTP-only. Browser/WebSockets and restart persistence remain
separate open selected-board gates.

BM1397 alternate exact crate download failed DNS (curl6), and the exact0.2.0
register.rs/core_register.rs source URLs failed in the web provider. Evidence:
bitaxe/bm1397-source-access-156.json and its referenced curl log. No source bytes
or new hardware behavior were inferred. Do not repeat this unchanged failure.

## 02:24 UTC22October — both board150 failures retained; measured low-rate fix158

Both source150 board runs FAILED after about2333s of actual inner execution.
Full results/traces are extracted under network-vm/board-{ssh,local}-dashboard-150/
board-result. Both strict regtest audits reject firmware runner exit1; no Core
node test was attempted. Local: first authorized pool session submitted one
accepted result at518.04s, changed difficulty/job, then timed out at600s before
second acceptance. SSH: first authorized session timed out600s with0submissions.
Other V2-looking connections remain retained and do not replace these failures.

The separate hash diagnostics preserve failure status. Independent ASIC replay
checked5724local and9032SSH computed hashes. Local real first-job hashes2357,
second-job hashes4; SSH real first-job hashes1377, no ASIC filter results for
that work. The remaining hashes include dummy work. These diagnostic reports
explicitly grant no mining, share, block or boot acceptance.

Local HTTP readiness never returned native HTTP200. SSH reached the exact
2044-byte native index, then the425409-byte JS asset timed out after164816bytes
in10.01s. The selected-dashboard report preserves that actual failed assertion.
No dashboard pass. Both runtime bridges closed with healthy supervisors.

Source157 added a60s per-request transfer budget only to the selected nested
native probe; standard client remains10s. The full asset hash and all native
assertions are unchanged. It also records actual pool CLOCK_MONOTONIC start,
end and remaining budget for future timing analysis. A misplaced evidence-log
list in an SSH startup call caused157compile/Clippy failure. Source158 fixes
that placement. Source158 full458tests/95binaries, Clippy/fmt and actual selected
TLS transport fixture PASS. New fixture additionally proves the actual worker
rejects a different complete ProcessSpec that shares the original input bytes.

Next board preparation uses source158, the same2400s execution profile,
900s pool window, cap1000, guest clocks/bus timing unchanged. Explicit fixed
ASIC filter adaptation is1bit instead of4; the guest's programmed ticket bits
and readback are unchanged. Pool difficulties change to-31,-30. Lowering only
the fixed filter while keeping the old pool target would not improve their
intersection probability. Both constraints are relaxed explicitly for very-low-
rate functional mining. All returned candidates still require actual hashes,
independent header verification and per-job Core acceptance. This is an adapted
reference path, never an original/exact-board pass. Preparation output:
qemu-s9/worker-prepared-dashboard-158. Native TLS is part of the SSH client now;
source158 composition still requires actual board execution.

## 02:41 UTC22October — persistent command pool159 passes component checks

Source159 full468tests/95binariesPASS, Clippy/fmt and release buildPASS. Frozen
source159 preserves359files. New explicit pool-keep-open prepares schema6 and
uses the same independent verifier over inherited command pipes. Older inputs
retain their existing policies. Tests exercise two fresh jobs, clean transition,
difficulty changes, a third valid share and invalid/duplicate/stale/range errors,
plus peer, deadline, partial-frame, cancellation and budget shutdown. The runner
now rejects a session that completed work but later failed, stopped or exhausted
its budget. These are component tests; schema6 firmware execution is pending.

Both source158 actual board runs remain active (started02:25UTC): sessions95614
SSH and6077local; network-vm/board-{ssh,local}-dashboard-158. Same prepared
ProcessSpec file2cf0f1434680fab6a5897647cbe7a5ba0368c6f31f893ed46d5bd1485adac651
/10316bytes. Selected SSH run is
run-771076252b37d92a74ae5f5f89f65dc22263c46d1ff3c3465b2cbd66fcd29e35.
These frozen158 runs do not include159 persistent pool. Final mining, native
HTTP/TLS and independent audits are pending. Next: selected Boser browser
integration with real services, captured Firefox and isolated profiles.

## 02:58 UTC22October — selected browser162 deployed; strict UI review finding

Source160 full470tests/95binaries, Clippy/fmt and release buildPASS. The actual
isolated browser-group160 VM also PASS: SIGTERM to a separate supervisor kills
both owned browser cgroups and a detached setsid descendant. Signal uses only
async-signal-safe writes through dedicated CLOEXEC cgroup descriptors. Both
leaves empty; raw supervisor exit143; no firmware/browser acceptance granted.
Evidence: browser/group-check-160/signal-cleanup-report.json and captured result.

Review caught the160 ready-record omission that would break the older local
browser supervisor. Source161 restores its original origin field. Its43affected
unit testsPASS and source snapshot retained. Source161 was not used for a board
run. Source162 additionally records the exact archived Boser app's hard-coded
ws:// URL. Selected TLS now explicitly uses the existing gateway
upgrade-insecure-requests header policy. No original asset is edited. Review:
reviews/boser-websocket-policy-162.json. Previous browser tests proved native
login and separate real GraphQL telemetry, but did not require populated native
DOM tables. Their initial DOM can contain placeholders. They do not close the
full native UI gate. New code requires three63-chip100MHz rows and the selected
private pool row with accepted work, in addition to all prior service checks.

Source162 builds and Clippy pass. It has362frozen source files. Browser client
starts concurrently with the selected firmware, then waits for exact validated
LockedRun/ProcessSpec/Binding plus actual HTTP/TLS preflight. It uses two fresh
Firefox profiles at each selected origin; remote tests use the same profiles
across HTTP and HTTPS for this one run. Plain HTTP is the actual SSH tunnel;
HTTPS checks every GraphQL/asset response's exact selected-run header. Complete
run-policy, input, binding and publication changes are covered by negative tests.
The original local support-VM mode keeps its existing report/readiness contract.

Fresh prepared process162:
794cad752df2c3369fc44dbc4df1a4010931f444d0e46a8f64d67646fd28042e /10316bytes.
It uses explicit schema6 pool-keep-open, cap1000, fixed ASIC filter adaptation1,
pool difficulties-31,-30,900s pool and2400s nested execution, backend130d,
baselineSD004 and original-monitor adaptation. Same process deployed locally
and over SSH. Actual runs started02:58UTC:
- network-vm/board-ssh-dashboard-162, session41653.
- network-vm/board-local-dashboard-162, session49545.
Both use the captured native-driver-024 browser support payload with current
helpers. Topology is private worker/client namespaces in a disconnected local
VM, not an external physical host. Final browser/mining/Core results pending.
Source158 runs remain active with frozen158 helpers; do not use162 reporters on
those results. Agents remain service-quota blocked; independent review gate open.

## 03:04 UTC22October — failed162 image preparation; bounded fix163 running

Correction to the preceding launch entry: neither162 board run reached guest
execution. Both stopped with `payload files exceed 1GiB`. Exact captured browser
support plus the prepared board inputs exceeds that support-image bound. The
failed outputs and logs remain. Source162 full472tests/95binariesPASS.

Source163 raises only the support payload bound to1280MiB. The combined image
bound remains2GiB; guest, browser and process execution budgets are unchanged.
A boundary/overflow test rejects bytes beyond the declared support-image cap.
Source163 full473tests/95binaries, Clippy/fmt, release build and445guide linksPASS.
README now states the observed native DOM coverage gap and recorded gateway
header adaptation. Frozen source163 retained; this README text was added after
that source snapshot and does not alter executable inputs.

Fresh prepared process163:
f4af04314c33ce83327656df9a6a6062ff7ee4b9795c3bb2b725f9fa0dbc4c57 /10316bytes.
Same flags as162 (including schema6/pool-keep-open), fresh seed. Runs started
03:04UTC: network-vm/board-ssh-dashboard-163 session17253 and
network-vm/board-local-dashboard-163 session3443. Both still preparing/starting;
no firmware, mining or dashboard pass yet. Source158 is near its declared end
and still needs strict result extraction, independent audit and Core tests.

## Source158 local complete adapted-reference path; SSH support-CPU failure

Local158 actual outer and inner processes exit0, complete traces. The frozen158
namespace reporterPASS and selected native HTTP suitePASS (148readiness probes).
Strict original-firmware submission auditPASS:2fresh jobs, clean transition,
difficulty change,6255independently replayed computations,25valid serial results,
5matched submitted computations. Both separate Core candidate testsPASS with
height0->1 and restart persistence. Blocks:
3563db2002971024c1ca2751d3e5325295a24ff78d248bd4d0ea9c00d5bce254
1196a27801d48eef0fe1a0b123a8cae89af10bbc2b661d1ec93e52ad685a3916.
Evidence: network-vm/board-local-dashboard-158/remote-report.json,
board-result/{regtest-audit.json,selected-dashboard.json}, and
bitcoin-core/board-local-158-job-{0,1}/block-acceptance.json. These are actual
archived firmware in adapted reference hardware, not exact physical targets.
The local run did not test the selected firmware over SSH/TLS or a browser.

SSH158 outer QEMU failed withSIGKILL and complete wrapper resource diagnostics:
wall2424260ms, user3588555622us+system30121825us (~3618.68CPU seconds), RSS2348152KiB.
Its enforced CPU cap was3600s. It stopped before final namespace/result transfer,
so no complete inner result or mining/Core/dashboard pass is available. Retain
all provisional traces; do not export them as a complete result. This measured
support-VM budget failure is separate from native firmware correctness.

Source164 sets bounded outer2-vCPU budgets to2*wall:4800CPU for2400wall and
6000CPU for3000wall, with distinct confinement guards. Existing1800/3600 profile
and all selected inner firmware/bus/boot/mining budgets remain unchanged. Build,
freeze, Clippy and tests are in progress. Source163 local/SSH VMs did start after
image preparation. They retain their old3600CPU cap and will provide independent
browser diagnostics; their final outcomes are still pending.

## 03:12 UTC22October — corrected outer CPU profile164 deployed

Source164 full473tests/95binariesPASS, Clippy/fmt and release buildPASS. Frozen
source164 includes the README coverage correction. Exact fresh prepared process:
b36ac9d4d1563a51a4dc34210556a5e91684f272d48bbec4c953d3e155997f5b /10316bytes.
SSH board164 with captured browser started03:12UTC, session52670, at
network-vm/board-ssh-dashboard-164. It uses outer wall3000/CPU6000 and the distinct
slow confinement guard. Inner process remains2400wall/2390CPU, pool900 and the
same explicit low-rate adaptation, with a fresh seed. Native browser, mining,
strict replay and Core results remain pending.

Source163 selected IDs observed after actual boot/transfer:
local-3d2829f604041298482408480e46825caff06b907d845c85cd327ac226775134;
run-8ab8331508c68d975ab9c547686943b854e6a638dc5e56e51ad53d5407f00622.
These runs remain active and still use the old outer3600CPU cap. Their frozen
inputs and reporters must stay separate from164.

## 03:38 UTC22October — timed sensor plans and checkpoint correction166

Implemented bounded, strictly ordered TMP421 external-input plans in Rust,
through the BM1387 chain, Rust C interface, and QEMU property. Plans use virtual
nanoseconds. Bus/conversion completion precedes an equal-time input change.
Power-off does not restart the external plan; time-origin reset replays it.
The original plan bytes, canonical argument and interpretation are retained by
s9_boot_bundle. No calibrated thermal model or native thermal protection pass
is claimed. Added sensor, chain-routing, C interface and preparation tests.

C linking exposed a checkpoint bug: cargo inherited CARGO_TARGET_DIR but the
helper copied target/release/libmd_sim_qemu_core.a. Checkpoint165 retains the
wrong old library059969b56556aff84354449fc8ce72eddf85b0886c1af16d3865f52b21f22e06.
The corrected helper reads Cargo's structured compiler-artifact message,
requires the exact source target/staticlib, rejects missing/duplicate/relative
paths, and hashes the emitted file before/after copy. Checkpoint166 retains
b10ac69c3705b15166d09cc44f2781944d73ef9381c4cba6ab23aa1a5553865d /33254598.
Review: reviews/checkpoint-artifact-path-166.json. Existing retained142/143
libraries match their current build output;144..165 differ;129..141 candidate
build outputs are unavailable. This comparison is not proof of historical
build provenance. Original reports and old checkpoints were not overwritten.
README now makes the source/library qualification explicit.

C interface166 passes ASAN+UBSAN with ASAN_OPTIONS=detect_leaks=0. The first
run failed because LeakSanitizer cannot operate under the environment's ptrace
restriction; leak checking remains unverified. Clippy166 and the new checkpoint
unit test pass. Full workspace tests166 are active(session67566). Release166
helper build passes. Actual QEMU UBSAN build started03:38(session64036), output
qemu-s9/backend-build-166-ubsan; full-backend tests remain pending.

Local163, SSH163 and SSH164 actual firmware/browser runs remain active; no
result.json exists at03:36. Their frozen inputs and source reporters remain
separate. Next: inspect build/smoke and browser outcomes, independently audit
completed mining, run Core for each job, then execute a real timed thermal
fault through the new backend. Full exact-target matrix remains incomplete.

Public web searches for BM1397 80008074, bm1397-protocol CoreRegisterControl,
and bm1397 core_register returned no results this session. No new protocol
semantics were inferred. Existing exact core-register blocker remains open.

## 03:44 UTC22October — compiled input plan passes; native run169 active

Backend166 UBSAN build and all startup checks PASS. QEMU binary:
35566c116cc450a2853527766aed7350e435610d4f0cada644d512a4ad30de72 /19925464.
Build uses corrected core-checkpoint166. The input-plan contract is under
bm1387_reference in the build report. An initial qtest helper looked for it at
the root; review fixed this before execution. Source168 retains the fixed test.
Compiled qtest168 PASS: plan1153 responses, baseline temperature1131, PIC585,
CPU timers112, fan25, NAND timing36, ONFI237, S9IO v1 1414. Each uses independent
literal response checks and retained serial traces. These are component checks,
not firmware or calibration passes. Plan test reads65,105,65 through real ASIC
serial packets and S9IO RX FIFO at explicit6s/8s external input edges.

First native thermal preparation166 failed before execution: the166 release
folder lacked isolated_network_vm and s9_pool_guest. Retained output/log remain.
Source169 adds named prerequisite checks and builds both support tools. It
retains corrected freeze169 plus full workspace478tests/95binaries PASS and
Clippy/fmt PASS. Source166 full478tests/95binaries also PASS.

Actual archived firmware thermal observation169 is active(session24039):
qemu-s9/temperature-high-169, helpers169, backend166, baseline SD004. Explicit
plan temperature-high-167.json sets remote105C at700s virtual and65C at800s.
Native defaults report hot100C/dangerous110C, so this tests the hot range only;
it must not be called a dangerous-overtemperature shutdown test. Longer derived
console-temperature-observation-167.sh collects original logread and native API
through sample240; it does not change native thermal policy. Direct local
isolated QEMU plus disconnected support VM, cap1000, filter1, pool−31/−30,
360s wall,230s pool, icount6. Actual outcome, independent mining and Core pending.

The three selected browser runs163local/163SSH/164SSH are still active. Agent
status confirms all three review agents remain unavailable under the existing
service quota until25October08:48. No repeated unchanged agent retry was made.
Next: collect actual169 sensor/native protection evidence, add dangerous and
sensor-fault cases; collect browser outcomes with matching frozen reporters.

## 04:05 UTC22October — native thermal results, remote defects, fixes174

Thermal169 with icount6 failed at291.622s wall: inner QEMU SIGKILL after
343.839 cumulative waited-child CPU seconds; observed ASIC time stopped at
691.620s, before the700s input edge. Its long console did not finish. No thermal
pass is granted. Retain temperature-high-169 and log. Actual direct-clock runs
170 use the known non-icount board path, helpers169/backend166, the same private
SD baseline and cap1000/filter1/pool−31/−30. Separate plans set105C or115C at150s
and65C at180s. The90-sample original-shell diagnostic completes. Hot170 exits0
at238116ms; dangerous170 exits0 at237055ms; complete traces for both.

Independent mining169 audits PASS: hot session0 two fresh jobs, clean transition,
difficulty change,90024 computed hashes,731 verified serial results,707 matched
submissions. Dangerous session0:66368 hashes,458 serial results,359 matched.
Dangerous restart sessions8 and11 also PASS with two jobs each and13/36 matched.
The new Rust temperature audit172 passes both runs. It joins actual guest ASIC
queries to CRC-checked sensor replies, verifies65→105/115→65 on all3chains,
no hashes while power/reset inhibit, and computation before/after recovery.
Dangerous170 native HALT messages explicitly report115C above110C; all3chains
cut power near151.7..151.96s, restart while hot and halt again near176.6s,
then repower189.5..189.7s and read65C. Hot mode grants readback/continued operation
only; fan-control threshold acceptance is not claimed by that audit.

Initial4 Core probes failed BEFORE Core execution: the complete submission
reports are3.0MiB/1.6MiB, above the old1MiB reader cap. Their directories/logs
remain. Source174 adds a32MiB bounded reader for both original and freshly
replayed reports, including type/growth checks. Candidate validation and exact
fresh-replay equality remain mandatory. Negative/large-input regression PASS.
Fresh source174 mining audits for hot0/dangerous0,8,11 are active, followed by
new Core tests for all8 candidates. No Core170 acceptance claimed yet.

Temperature audit uses shared completed-execution log integrity checks with the
mining auditor. Added negative tests for missing guest query, wrong controller
reply, inhibited hashing, missing shutdown/cut/recovery. Clippy173 found one
nonminimal boolean; fixed174. Source174 Clippy/fmt PASS; full tests active78502.
Source171 full479tests/95binaries PASS. Source173 targeted audit tests PASS.

Remote163local/163SSH/164SSH all FAIL. Matching frozen extract-board-failure
retains each complete inner result in board-result. All native HTTP readiness
failures reached status200 but curl28 with0/2044 body bytes. The suite incorrectly
made this partial response a permanent failure. New171 retries it within the
same readiness/process bounds; complete body hash remains mandatory. Both SSH
VMs also report10 OOM kills, including nested QEMU and Firefox, in3GiB guest RAM.
New selected-browser-only support profile:6GiB guest,4vCPUs,8GiB address bound,
3000s wall/12000s CPU; inner firmware2400/2390 budgets unchanged. New strict
confinement wrapper identifies that profile. Other workflow profiles unchanged.

Local171 and SSH171 started03:54UTC (sessions60573/20332). Both new VMs booted.
Prepared process f453b524033ac08e996c709300150ab836746c271404f552c9e6498efab469bb /10316.
Local selected ID local-ab8351a72f0c19a1bf3959d6d346ed2adeb148f3dfa2c4a958f0a4080c2cb40a.
Inputs backend166, helpers171, native-driver024 browser payload. Actual browser,
TLS, mining and Core results still pending. These are disconnected local VMs,
not external physical hosts. Frozen source171 reporters must collect them.

Thermal review also found PIC power trace timestamps were sampled AFTER a
potentially expensive connected ASIC callback. In dangerous170 a power transition
near151.7s printed the PIC event near158.2s. Source173 C binding now captures one
input timestamp per callback and prints the producer power edge before delivery.
No former cross-device timestamp equivalence is claimed. New UBSAN backend173
build active(session59852), checkpoint173; compiled qtests and new native thermal
runs are required after that trace fix. Current Rust source174 only simplifies
an unrelated audit boolean. Freeze174 remains to be made after checks.

TI TMP421 PDF remains unarchived. Web reader can open SBOS398C, but direct
curl failed DNS forwww.ti.com; log tmp421-download169.log. No PDF hash invented.
New static source extraction braiins-temperature-source-169 pins monitor.rs and
tmp42x.rs from archive dc5f5872...; it is a source reference, not proof of identity
with the proprietary2022 binary. All exact-board/full-matrix gates remain open.

## 04:16 UTC 22 October — Core acceptance and corrected power traces

All eight source174 Core checks for thermal170 PASS. Each uses an isolated fresh
node, imports a block from actual native firmware work, changes height 0 to 1,
and keeps the exact tip after restart. Paths are
bitcoin-core/temperature-hot-170-audit174-session0-job{0,1} and
bitcoin-core/temperature-dangerous-170-audit174-session{0,8,11}-job{0,1}.
Each block-acceptance.json retains the hash, replay and node inputs. Offline
block import was used; RPC was not tested. Earlier oversized-report failures
remain. Source174 full 482 Rust tests / 95 test binaries, Clippy and format PASS.
Corrected checkpoint174 is frozen.

UBSAN backend173 build and all startup checks PASS. Hash:
1510b3468910d45b8e8c4509923be09141708dd36108644ec38d6d774bf31a50 /19925432.
Compiled PIC qtest174 (585 checks) and temperature-plan qtest174 (1153) PASS.
New native temperature-hot-174 and temperature-dangerous-174 complete with
exit0, full traces, helpers174/backend173. Both temperature audits174 and
session0 independent regtest audits174 PASS. Hot has627 pool submissions;
dangerous has330, with additional completed mining sessions8/11 (10/53).
These new sessions still need Core checks. Dangerous174 shows the native115C
halt twice, all3 chain power cuts, recovery and resumed computation. The fixed
PIC producer trace now precedes each actual ASIC power edge, with separate
monotonic real-time timestamps. A strict Rust trace regression is being added;
no equality of the two real-time timestamps is assumed.

Sensor diode-open175 and ADC supply-invalid175 runs started with helpers174,
backend173, the same SD baseline and explicit150s fault/180s restore plans.
Sessions94026/25263. These are ADC validity faults, not total sensor power loss.
Actual native behavior and strict audits remain pending.

Local171/SSH171 selected browser tests remain active. Their guest logs show
continued native boot progress and no OOM message so far. No browser/TLS pass
is inferred. All three agents remain blocked by the same service usage limit
until25October08:48. Continue implementation locally; independent review gate
remains incomplete.

## 04:31 UTC 22 October — stronger sensor evidence and manual recovery tests

All eight Core checks for the corrected backend173 thermal174 runs PASS,
including dangerous sessions0/8/11. Each isolated node changes height0→1 and
retains the exact tip after restart. Reports:
bitcoin-core/temperature-hot-174-audit174-session0-job{0,1}/block-acceptance.json;
bitcoin-core/temperature-dangerous-174-audit174-session{0,8,11}-job{0,1}/block-acceptance.json.
Hot174:80367 computed hashes,652 verified serial results,627 matched submissions.
Dangerous174:66163 hashes,449 serial results;330/10/53 matched in sessions0/8/11.
All four sessions have two fresh jobs, clean transition and difficulty change.

Source175 adds a strict producer/consumer power trace check. Both thermal174
runs pass. The retained dangerous170 trace fails with "ASIC power differs from
the PIC output", as expected from the old trace-order defect. No old report is
changed. The corrected C binding's actual transitions are now verified.

Source176 adds paired sensor bytes and fault flags, explicit manual-recovery
checks and a frozen guest shell scenario. It passes488 Rust tests /95 binaries,
Clippy and format. One test closure initially inferred i32; fixed to explicit
u64 and rerun. Actual numeric replays then exposed repeated controller readback:
one completed I2C transaction is read more than once by the native driver.
The176 audit failed with "sensor low byte lacks a paired high read". Retain both
failed logs. Source177 tracks each applied guest I2C write and joins readbacks
by transaction sequence. A repeated result must be unchanged and cannot become
a new sensor measurement. Both hot174 and dangerous174 temperature-audit-177
reports now PASS, with paired high/low bytes, valid fault flags and power joins.
Source175/176/177 checkpoints are frozen. Source178 adds the captured repeat
regression and updates README/TMP421 evidence links; full checks are active85316.

Initial diode-open175 and supply-invalid175 complete with exit0/full traces,
238127/238075ms. All3 original sensors log "Too many sensor errors, disabling
sensor". No automatic recovery or protection pass is granted. The ADC supply
validity fault is not total sensor power loss. Retain all raw evidence.
First manual176 launch failed before execution because an explicit pool window
was restricted to nested mode. Source176 permits20..300s in the normal profile,
inside the existing process budget, with10s reserve. Nested20..900 policy and
71s reserve remain. Defaults, guest clocks and device/protection timing remain.

Manual recovery177 runs are active, helpers176/backend173, same150s/180s plans,
cap1000/filter1/pool−31/−30, explicit300s pool,360wall/330CPU. Sessions47576/54988.
The committed simulator/scenarios/s9-sensor-manual-recovery.sh calls the original
/etc/init.d/bosminer restart at sample100 after native uptime185s. It records
actual before/after PIDs. This is an explicit service action, not automatic
recovery. Native Linux uptime and QEMU time have separate origins; the audit
keeps them separate. Physical sensor recovery must include post180s power
cycling, healthy paired reads and new computed work. Audit/source hashes and
console-script identity remain mandatory. Results and Core are pending.

Local171 now reports selected-board-native-http-passed at1890110ms in its
support VM. Actual selected browser remains pending. SSH171 reports ASIC trace
activity at1925587ms. Neither has an OOM message so far. Do not replace their
selected IDs or frozen171 reporters. Full exact-target release gates stay open.

## 04:46 UTC 22 October — retained dashboard failures and sensor evidence

Source178 full489 Rust tests /95 binaries, Clippy/fmt and450 guide links PASS.
Source179 changes the manual recovery script to wait for one new native PID;
checkpoint179 freezes368 files. Targeted audit tests and release examples PASS.
Full179 checks remain to run. All three review agents still fail at the service
quota until25October08:48. Independent review remains incomplete.

Manual177 diode/supply runs FAIL. The original restart briefly returned two
PIDs. Both private support pools exhausted their16-connection bound near303s.
No failed result was changed. Source179 waits for one new PID; fresh180 tests
use a270s pool window and120 samples, inside360wall/330CPU. These changes affect
observation only; sensor edges remain150s fault/180s restoration.
Both180 runs exit0 with complete traces. Supply-invalid180 temperature-audit179
PASS, including all3 native sensor errors, paired fault reads, original service
restart, actual power cycles and new computation. Diode-open180 temperature
audit FAIL: a concurrent logread line appears immediately after the old PID in
the BEGIN marker. No automatic recovery or protective shutdown claim.
Independent regtest audits179 for both180 runs, sessions0/9, PASS. Actual matched
submissions: diode782/77, supply789/84. All sessions include two fresh jobs,
clean transition and difficulty change. Eight fresh Core checks are active.

Local171 and SSH171 both FAIL. Matching frozen171 extract-board-failure retained
board-result in each network-vm run directory. Selected local ID remains
local-ab8351a72f0c19a1bf3959d6d346ed2adeb148f3dfa2c4a958f0a4080c2cb40a;
SSH remains run-9ad3f3c7bf1ea442af5b15d11fe7734fb1671275948b40c22faa316e603d5729.
Both inner processes exit1 near2334s with "first authorized mining session
failed or ended incompletely". Actual first-job shares were accepted but no
second-job share passed before the unchanged collection deadline. No mining or
Core gate is granted. Local171 native HTTP PASS, then real Firefox Navigate
FAIL after90000ms. The native page remains interactive with no login/body;
1.588MiB vendors JavaScript is missing from completed resource entries. Native
app/CSS/small assets did arrive. Browser cgroup records no OOM. SSH171 never
reached a complete native HTTP response; browser/TLS not tested. This is a
private VM with worker/client namespaces, not an external physical host.
Next: test the existing real-time clock policy in fresh prepared/run identities;
preserve instruction-count171 results and all strict browser/mining assertions.

BM1397 still blocks on register0x3c value0x80008074. Public primary-source leads:
https://github.com/skot/BM1397/blob/master/core_registers.md and
https://github.com/GPTechinno/bm1397-protocol (crate0.2.0,6June2023).
The latter lists src/core_register.rs and src/register.rs; actual file/revision
acquisition is still missing. Direct raw.githubusercontent.com SVG acquisition
failed DNS, log bitaxe/bm1397-core-reference-179/download.log. Existing Mujina
mailbox reference coversBM1362/66/68/70 and is not BM1397 proof. No speculative
register acceptance was added. New exact source acquisition remains next work.

## 04:53 UTC 22 October — Core passes, framed markers and new clock runs

All8 Core checks for manual180 diode/supply sessions0/9 PASS. Each fresh node
imports a candidate from actual native work, changes height0→1, and retains
the exact tip after restart. Reports are under
bitcoin-core/temperature-{diode-open,supply-invalid}-manual180-audit179-session{0,9}-job{0,1}/block-acceptance.json.
This does not change the failed diode180 temperature marker audit.

Source181 adds an explicit end delimiter to each recovery marker. The verifier
requires that delimiter and numeric fields. It rejects the retained damaged180
marker, repeated records and multiple PIDs. Background log text after a complete
framed record is separate. The native-error boundary now uses an emitted line,
not a marker substring in transmitted shell source. That echoed-source case
has a regression. Final source181 build,489tests/95binaries,Clippy/fmt PASS;
452 guide links PASS. Checkpoint181 freezes368 files. Fresh diode/supply181
runs are active, helpers181/backend173, same270s observation and150/180s edges.
No recovery pass is inferred until the new complete traces are audited.

Prepared realtime179 ProcessSpec:
a45217ba82e4b58445f281cd8534b358309a607bcca0fe73704845175fdfa9a0 /10266.
Inputs helpers179/backend173, native-driver024 payload, baselineSD004,
console133, cap1000, ASICfilter1, pool−31/−30, pool900,2400wall/2390CPU.
The existing normal QEMU real-time clock is selected; no instruction-count flag.
Local and SSH use the same prepared input and separate privateVMs. Runs:
network-vm/board-{local,ssh}-realtime-179, sessions52235/51785. Preparation
completed and VM execution started. No external physical host is involved.
Matching frozen179 reporters must collect them. Full179 checks also PASS489/95.

Source182 adds a shorted-diode audit case. It requires paired extended-range
−64C bytes, clear open/invalid flags, native error messages, original manual
restart, real power cycles and restored computation. The source reference
braiins-temperature-source-169/tmp42x.rs reads both bytes and treats high0 as
short circuit; its pinned archive/member hashes remain separate from the2022
binary identity. New test run temperature-diode-short-manual-recovery-182 is
active with frozen181 execution helpers/backend173. Its auditor182 build and
full checks remain to finish. No short-diode acceptance is claimed yet.

Public BM1397 diagram viewer resolves commit
6798f0512cc74bbcf138077c6b41dbc083577ea8 in its exact render URL. The SVG itself
remains inaccessible via direct/API/web retrieval; no source-file hash is
claimed. GPTechinno core_register.rs/register.rs reads also fail with cache
misses. Preserve this lead; do not repeatedly retry unchanged access failures.

## 05:08 UTC 22 October — sensor passes, cooling evidence and pool capacity fix

Supply-invalid181 and diode-short182 complete with exit0/full traces and pass
source182 manual recovery audits. Source182 is frozen;489tests/95binaries,
Clippy/fmt PASS. Actual first/recovery mining sessions are0/10 (704/60 matched)
and0/9 (688/86 matched). All four independent regtest audits182 PASS. All8
Core candidates PASS separate fresh imports, height0→1 and restart persistence.
Reports: bitcoin-core/temperature-supply-invalid-manual181-audit182-session{0,10}-job{0,1}
and temperature-diode-short-manual182-audit182-session{0,9}-job{0,1}.

Diode-open181 FAILS despite intact framed markers and successful native script.
The support pool exhausts its16 lifetime connection slots, leaving actual
recovery session11 incomplete. Session0 had745 submissions, but no overall
mining or recovery pass is granted. This is a real capacity defect, not a
sensor marker defect. All unsupported native V2 probes remain in the record.

Backend183 adds QEMU virtual timestamps to fan MMIO traces. No motor outputs,
state or timing parameters change. UBSAN binary:
e052122055aed80fabc9a7c851088adb7063d77fc5e5d140ecee36a74b2921ab /19925496.
Fan binding87678bc5008a3d1e6c8e9460f3604f52af4c080dab7798aed0f27047331905ef /5986.
All12 backend startup checks PASS. Fan and absent-fan qtests184 PASS25 exact
responses each, including timed motion, stall/power/reset. Source183 is frozen.

Source184 adds a separate `s9_asic_audit cooling` operation. It retains all
numeric thermal assertions, then checks the exact fan binding and fixture,
paired native hot/healthy sensor reads, both full PWM commands, sustained
full duty, settled6000RPM tach readback, later duty reduction and slower motion.
Missing clocks, wrong fixture, duplicate fields, changed responses and wrong
ordering fail regression tests. No physical response deadline is claimed.
Source184 is frozen369 files;491Rusttests/95binaries,Clippy/fmt PASS.
Native temperature-hot184 (helpers184/backend183) exits0; cooling-audit184 PASS.
Both PWM1→100 after~0.994s from first105C read; connected tach100RPS at153.383s.
After healthy read180.685s, both PWM→88 at182.231s; tach74RPS at184.561s.
Independent mining audit184 session0 is active, then two Core candidates.

Source185 is under test, NOT FROZEN. New pool schemas7/8 add an explicit hashed
connection_limit (default128, accepted16..128). Legacy2..6 require no newfield
and retain16 slots. Both transports use the declared lifetime limit; TCP still
permits at most16 concurrent sessions. Seed derivation is unchanged, extends
only through the declared slot count, and remains distinct per connection.
The collection verifier checks the exact expected budget and keeps first
actual authorized V1 selection independent of success. Missing/incomplete
sessions and failed work still fail. The Core bridge permits the128 global
bound and requires its exact fresh auditor replay to check the per-input bound.
A legacy test first failed because it retained the new field while pretending
to be schema2; the fixture now removes it. Failed logs stay. Corrected full
checks/build are active. Fresh diode execution must follow a frozen185 build.

Realtime179 local/SSH runs remain active with selected IDs
local-f56e5fb6be8f940c3fe1583eb3aa1b799ab09f6a5a96bb33b5d8ae81494d74f3 and
run-3edc1f5f338adee9dc4453d2f8e6b602c9b8df3905258121e446b44efae8c828.
Last support elapsed~832s. Native Linux has booted and reached network config;
"Command failed: Not found" appears during early native service startup.
No native dashboard or mining pass is inferred. Keep matching179 reporters.
All exact-target, original-board, independent-review and complete-matrix gates
remain open. Agents still unavailable under the unchanged service quota.

## 05:21 UTC22 October — fault recovery and explicit connection coverage

Source185 is frozen in core-checkpoint-185. Final checks pass504Rusttests in95
binaries, Clippy and fmt. All three fresh native fault runs185 exit0 with full
traces and pass temperature-audit-185: diode-open, supply-invalid, diode-short.
Each has actual completed V1 sessions0/9, before and after manual service
restart. Matched submissions are785/75,762/85 and709/94 respectively. All six
fresh independent mining audits185 pass two jobs, clean transition and changed
difficulty. All12 Core candidates pass separate imports, height0→1 and restart
at1. Evidence is in qemu-s9/temperature-{kind}-manual-recovery-185 and
bitcoin-core/temperature-{kind}-manual185-audit185-session{0,9}-job{0,1}.
No automatic recovery, physical calibration or exact-board gate is inferred.
The explicit128-slot collections retain all14,15,14 actual connections.

Both hot184 Core candidates also pass import and restart. Their reports are
bitcoin-core/temperature-hot-184-audit184-session0-job{0,1}/block-acceptance.json.

Source186 strengthens complete pool collection accounting: allocation IDs
must cover0 through N−1 exactly, even for failed probes. Missing IDs or repeated
IDs fail. Its full checks pass508tests/95binaries, Clippy/fmt. It adds a32-V1
connection transport fixture plus one independent binary probe. First execution
network-vm/pool-reconnect-186 FAILS at the first invalid submission. The client
still expected legacy result=false; actual schema7 JSON-RPC1 correctly returns
result=null with error20 and exact message. The failed guest log and trace stay.
Source187 fixes the assertion to require the entire exact JSON-RPC1 response.
Its build is active. This transport fixture cannot count as firmware mining.

Public CDN and docs.rs alternatives for BM1397 core-register source also fail
access. No new source bytes or file hash are claimed. Existing exact unsupported
boundary0x3c=0x80008074 remains. Do not retry these same failures without change.

Selected realtime179 local/SSH runs remain active. Native services progress
slowly under nested TCG; no dashboard or mining pass exists yet. Retain their
selected run IDs and use frozen179 reporters. All broader completion gates
remain open.

## 05:30 UTC22 October — connection test fixed; timed fan fault path

Source187 is frozen. Its508tests/95binaries, Clippy/fmt pass. The32-connection
fixture187 exposed a second defect: thread creation fails after28 completed V1
sessions because completed JoinHandles retain resources until the pool window
ends. Source188 joins completed threads during the accept loop, counts lifetime
allocations separately and uses fallible thread creation. The active16-session
bound and declared lifetime bound remain. `network-vm/pool-reconnect-188` PASS:
all33 connections (32V1 plus one binary probe), all invalid-submit rejections,
input-derived per-session seeds, complete contiguous allocation IDs and packet
captures. No firmware-mining gate is inferred. Failed186/187 results remain.
The failed186 pool source was reconstructed from187 by reverting the assertion;
its hash exactly matches the hash emitted by that executed binary. See
pool-reconnect-186/source-reconstruction.json. It is labelled reconstructed.

The new complete-collection verifier187 also passes fresh replay on all six
sensor185 sessions. Twelve Core rechecks187 are running; the earlier twelve
Core185 checks remain separate passing evidence.

Source188 is frozen in core-checkpoint-188 (372files). Rust512tests/96binaries,
Clippy/fmt pass;457 guide links pass. It adds a Rust timed external fan-input
plan, strict ordered/bounded JSON, absolute virtual timestamps, no fault replay
on controller reset, and applied-event records. C adds only the QEMU property,
FFI and trace binding. Power loss coasts; a locked rotor stops immediately;
actual integrated motion supplies tachometer reads. Analytic pulse tests and
irregular/late-callback/reset tests pass. The boot helper retains original plan
bytes, hashes and explicit adaptation metadata. Compiled fan-plan qtest is
implemented but has not yet run. Backend188 UBSAN build is active (session71521).
Next: require all backend startup checks, run fan/fan-empty/fan-plan qtests,
then actual firmware with fan-stall-input-188.json and fan-power-input-188.json.
No native fan-fault result is claimed yet.

Realtime179 selected local/SSH runs still execute under nested TCG. SSH reaches
the native uhttpd service; neither has a dashboard/mining pass. Last support
elapsed about2138s. Keep matching179 reporters and original selected run IDs.

## 05:45 UTC22 October — remote deadline evidence and new fan recovery audit

Both realtime179 selected board runs FAIL at2400s, without truncation. Matching
179 reporters extracted each board-result directory. Both native services were
still starting; SSH had bosminer PID2167 but its API refused connections and
the retained pool collection has zero sessions. Neither browser was selected
for its native UI, and no TLS publication pass exists. Do not count their
outer guest failure as an original firmware or dashboard pass.

Source189 is frozen372files. All512Rusttests/96binaries, Clippy/fmt pass;
459guide links pass. It adds a distinct nested-tcg-4800 profile:
inner4800wall/4790CPU, unchanged900s pool window and ordinary guest timers.
Its computed boot allowance is3830s. The support VM is5400wall,21600CPU with
four vCPUs for browser tests,8GiB address limit and6GiB guest RAM. The separate
non-browser allowance is10800CPU. The browser's explicit selection wait is4900s;
its active browser test deadline stays360s. Earlier2400 profiles stay unchanged.
Prepared189 ProcessSpec e5f04072c0db24951a339e5ed601f1be87bd4013eca93ce93bed9423d136f03f/10267.
Same prepared input is now running locally and through SSH in separate local
disconnected VMs, sessions43743/23910. Selected IDs:
local-a51ca0f333ba219a09b8c42c9c102554184ccc03b9a42d20b60056f77b07c083
run-4afe9b17675a2f481155d192303d6af61a0bcf3ff81f4ad5d3a9122a219d556d.
Use frozen189 reporters when complete. No external physical host is involved.

Backend188 UBSAN passes13 startup checks. Binary70de00c120434c3d39757a6efb39cae23f08c7d0f965889b8d0d3e6a03ff93d5/19937912.
Connected and absent fan qtests pass25responses each; timed input qtest passes
21responses and all four exact applied event records. All twelve sensor185
Core rechecks with auditor187 also pass. This is still the earlier auditor's
scope; see the correction below.

Native fan-stall188 and fan-power188 exit0/full traces. Both apply exact150/180s
inputs, log 'Shutdown: not enough fans (0 < 1)' and initially cut all three
chain power outputs after real zero-tach reads. Full automatic recovery is NOT
established: stall has only completed session0,343submissions; power has0/8,
327/128submissions, but chains6/7 do not resume hashing in that observation.
Stall repeatedly restarts, then begins another initialization near200virtuals.
These are not complete fan-recovery passes. Their traces stay unchanged.

Manual fan recovery190 is active for stall/power (sessions5602/26514), helpers189
and backend188, pool300s, same input edges, original service restart. The new
scenario changes only SENSOR→FAN marker names from the sensor script. Its hash
is8fa22bc67ff70ecd3f0daa6239b28fca19cac6a37e281247ee036015d0e0e6ae/4315;
provenance is in simulator/scenarios/s9-fan-manual-recovery.provenance.json.
Auditor190 adds a strict fan-recovery operation with pinned plan, script,
compiled binding, actual applied inputs/tach reads, native fault text, all-chain
initial power cuts, paired healthy readings and renewed computation after
manual power cycles. It separates guest uptime from emulator time. Build and
full checks are active; no fan recovery pass exists yet.

Review correction: source187's new complete-collection check applied to the
runner, NOT the independent submission auditor. The auditor still had a stale
16-session selector limit and did not compare the collection marker itself.
Source190 now uses the exact frozen input limit, checks the actual complete
collection and contiguous IDs, and still selects the first actual authorized
session independently of success. It adds a high-ID regression. Earlier185/187
mining and Core evidence must not be described as validating that new collection
check. Fresh affected replay/Core checks190 remain required. An initial190
lint failure from an unused shared marker was fixed by using that marker in
the parser; failed log remains. Current source190 is not yet frozen.


## Continuation191 — independent replay and interrupted PIC parser

Source190 is frozen in core-checkpoint-190,375files. Full516Rusttests/96binaries,
Clippy/fmt and release examples pass. The corrected independent submission
verifier passes all six sensor185 sessions (0/9 in each case) and fan-stall190
sessions0/10/13. Eighteen corresponding Core checks190 are active. Earlier
185/187 checks retain their earlier scope.

Fan-stall-manual-recovery-190 exits0 with full traces. Fan-recovery-audit190
passes initial shutdown and explicit manual restart/recovery on all3chains.
Restart markers use guest uptime202.41/204.75s and PIDs2466/2545. All3chains
compute after the final native power cycles. This does not establish automatic
recovery, physical calibration or an exact-board pass.

Fan-power-manual-recovery-190 times out360s. The exact cause is retained at the
end of backend.log: PIC57 sees55 at221946507570ns after an interrupted55 AA
prefix, returns model error155, and stops QEMU. The outer timeout is a failure,
not merely slow boot. Source191 traces the exact application words and adds
its one-byte invalid-command path for00/55 in application mode only. Other
unsupported commands and loader55 remain errors. See
simulator/firmware/pic16/parser-analysis.md. New Rust, C ABI and actual AXI IIC
regressions checkEE, unchanged power/timer/flash and the native flush. Full
checks/build are active. No repeated native power-loss pass exists yet.

Both selected realtime189 local/SSH VMs remain active at native boot. They use
the same selected IDs and prepared189 inputs recorded above. No dashboard,
TLS or mining completion is inferred. The three review agents remain blocked
by the service usage limit until25October08:48; independent review stays open.


Source191 pre-freeze review correction: the first draft mapped the app'sEE
write to I2C. Primary Microchip DS40001715D register maps show019a is UART
TX1REG; I2C SSP1BUF is0211. The draft was corrected before backend build or
firmware execution. No I2C reply is now supplied. Rust/C/bus tests require an
empty I2C reply queue and a later actual version command. The diagnostic UART
remains unconnected and outside this contract. The first draft's test log is
retained as pic191-initial.log and is not evidence for the corrected behavior.
Full checks and final191build must be repeated after this correction.


Source191 is now frozen376files. Corrected517tests/96binaries,Clippy/fmt and
463README/docs links PASS. The C ABI test passes ASAN/UBSAN with the exact
191static library; LeakSanitizer disabled as before. Backend191 UBSAN is
building from188 plus the191library. The new compiled bus test still needs
to run. All18Core190checks PASS,height0→1,restart1: twelve sensor185 candidates
and six fan-stall190 candidates. No power-loss recovery or remote189 result
passes yet. The new GitHub tagged-file view for BM1397 also failed to fetch;
no new protocol file identity is claimed.


Source192 review fix follows191: exact app words0478..0481 test zero only for
command15. The old range check falsely rejected enable values2..255. The fix
accepts all nonzero values and preserves the transient enable when a new
host's55 prefix arrives as the old command's payload. Exhaustive255-value
Rust regression and C/compiled-bus noncanonical55 tests were added. No state
or timer success is invented. Backend191 remains its separate frozen build;
192fullchecks/releasebuild/freeze/Csanitizer are active. The next actual
power-loss run will use the final192backend after its bus checks.


Source192 frozen376files,518tests/96binaries,Clippy/fmt/463links,C ASAN/UBSAN
PASS. Backend191 passes13startupchecks and the new parser bus qtest191. The
same191qtest FAILS on prior188 as required: it exposes the actual unsupported
interrupted prefix. Backend192 build is active, session10587. Its additional
nonzero-enable bus test will run before native fan tests.

Source193 auditor review adds native new-PID chain initialization evidence and
console marker order to both fan and sensor manual recovery audits. A printed
PID transition alone did not establish that the replacement initialized all
three chains. Negative regressions reject old-PID, missing-chain, pre-restart
and reverse-marker evidence. Device clocks remain separate from guest uptime;
no cross-clock equality is assumed. Full checks/release/freeze are active.
Earlier audit reports retain their scope and need fresh193 replay for the
new requirement. This change does not alter the192device model.

The archived DCENT sim_pic16_runtime.rs was inspected as a reuse lead. It is
a driver/behavioral-model test, not a PIC CPU emulator. No PIC instruction
execution claim is inferred from its filename. Parent archive hash remains
5501e53fc05648ce4a03da2b47ba799c0fe1e0c6dd4e358db5a18504d419f25c.


Source193 frozen376files;519tests/96binaries andClippy/fmt PASS. The tightened
sensor audit passes all3sensor185 runs; tightenedfan audit passes stall190.
These are new recovery audits, not a rewritten mining/Core190 claim.
Backend192 hash2fb6de0f975d90ad4dacad8002ddb13f5b58e99749d8af9374d6ae787846111d/19937912,
all13startupchecks PASS. Its compiled PIC qtest passes875protocolresponses
plus6IRQevents, sevenexactpowertransitions. New192qtest fails on191 due to the
old nonzero-enable range check. Separate191qtest fails on188 due to the old
interrupted-prefix failure. All negative results remain retained.

New native fan runs now execute helpers193/backend192, same pinned script and
150/180s plans,360wall/330CPU,pool300s,cap1000/filter1,baselineSD004:
fan-power-manual-recovery-193 session18760;
fan-stall-manual-recovery-193 session12114.
Next: inspect complete actual traces, run193fan-recovery audits, enumerate
all actual completed pool sessions, independently replay freshwork and submit
two candidates per selected completed session to separate Core nodes. Failed
power190 remains a failure and is not relabeled by a later result.


## Continuation194–195 — pool lifetime and sensor format audit

Both fan193 runs exit0 with full traces on backend192. Their independent
fresh-work193 audits pass sessions0/10/14 in each run. Twelve corresponding
Core checks are now active (session70223). The recovery audit193 fails on an
actual sensor format transition. Chain8 in power193 returns remote high41
(normal65C), after native config09=04 selects extended range; the native
interpretation is1C at212076676975ns. TI SBOS398C page11 states that format
changes occur at the next conversion. The reference conversion schedule must
be checked independently before any exception is accepted. All fault flags,
input identities and settled temperatures remain required. Diagnostic195
source and binary are preserved separately; no recovery pass is claimed.

Source194 fixes a TCP-pool lifetime defect: a short successful console script
could drop the live miner while the support pool was still collecting work.
The existing bounded pool-completion wait now also applies without an SD
checkpoint. Source194 is frozen376files and build/checks pass. Actual low-rate
validation is pending. The first two cap10 runs (native-low-rate10-193 and194)
used the wrong first-boot script with a persisted SD baseline, and exited1.
They do not test the low-rate path or establish this fix. Correct run
native-low-rate10-baseline-194 uses the baseline-compatible console133
c7cbba85e2eb96bc3ebde8e3eb56901f930db3179b53a102246029bb8c50502e/3301,
backend192, cap10H/s, filter1, pool-31/-30, pool300,360wall/330CPU.
It is active as session57775.

Selected realtime189 local/SSH runs remain pending. Three independent agents
remain unavailable due to the service quota until25October08:48. No unchanged
quota retry is attempted. The independent review gate remains open.


Source195 range oracle is retained in core-checkpoint-195-range1 (376files)
and ssh-target-195/release/examples/s9_asic_audit. Its two new regressions
reject late normal-format bytes, absent configuration, repeated same-value
writes, faults and unrelated input changes. It uses an independent arithmetic
conversion schedule from the pinned reference profile, not the model Sensor.
Both fan193 recovery audits195 PASS. Power193 has one transient on chain8;
stall193 has one on chain7. Actual readings remain in the reports.
This is manual recovery in the adapted reference environment, not an exact
physical board or automatic recovery claim.

Correct low-rate baseline194 exits0. Independent195 session0 audit PASS:
1318 actual computed hashes,12 matched submissions,2 fresh jobs, clean-job
transition and difficulty change. Twelve submissions include real rejected
low-difficulty results. Two independent block candidates are retained; Core
checks remain pending. Collection completed10 sessions, one completedV1
and nine native binary-protocol probes.

Parallel fan193 Core checks encountered native Resource temporarily unavailable
errors. These remain failures. A serial replay with unchanged node/input
identities uses new -serial output directories; first three retries PASS.
Do not count the failed parallel output as accepted. The runner process limit
and parallel Core launch interaction need review. Source196 additionally
checks exact sensor write data readback and actual bus completion time, asserts
that the reference timing profile matches the independent oracle, and records
the sensor transaction verifier identity. Full checks and release build active.


Source196 is frozen in core-checkpoint-196,376files. All522 Rust tests in96
binaries,Clippy/fmt and release examples PASS. Sensor recovery replays196
pass all3sensor185 runs; fan recovery replays196 pass bothfan193 runs.
Both low-rate194 candidates pass Core196: height0→1, restartheight1.
All12fan193 candidates now have passing Core193 results. Six initial parallel
attempts failed: three recorded Core nonacceptance; three stopped earlier.
The six -serial replay outputs pass. Failures remain retained.
The low-rate pool-work196 audit is running to record the exact intersection
of programmed ASIC and pool filters, in addition to the completed submission
audit. README contains the tested cap10 invocation and currentfan results.
Docs snapshot196-docs precedes a minor spacing correction; code is unchanged.

BM1397 alternate docs.rs source URLs for bm1397-protocol0.2.0 are inaccessible.
The skot raw registers.md endpoint returned text, but its required core-control
and core-value SVG diagrams still fail with Cache miss. The repository text
does not contain their bit positions. No mailbox behavior is inferred from
other ASIC generations. The selected Bitaxe path remains explicitly blocked
at actual command0x3c=80008074; no unsupported transaction is bypassed.


Source197 is frozen376files,523tests/96binaries,Clippy/fmt PASS. It fixes
the pool-work diagnostic's arbitrary65536 issued-job limit. The complete
packet/hash audit still checks every job and computation. Prefix search now
visits every actually computed midstate/tail, with the same262144 unique-state
bound; unused work does not consume this search. Regression checks a65537-job
history and invalid job/midstate references. The actual low-rate trace has
68877 issuedjobs,1318hashes,1294computedstates. It correlates1269 fullheaders;
49 computations remain explicitly unmatched. Initial25 resemble native open-
core nullwork; later24 have a different nBits. That is a diagnostic inference,
not a compatibility or mining pass. No unmatched hash produced one of the
12 independently verified submissions. The source197 report lists their
indices. Source198 is adding executed-cap identity and a conservative whole-
run count/rate bound, with separate statement that per-hash spacing is not
measured by that bound. Full checks/build active; no198pass yet.

Local/SSH realtime189 remain active around3300s outer elapsed. Original
logread reports Failed to find log object during early boot, so native
messages are missing from this provisional sample. The pool sees repeated
connections, some closing after configure; no completed mining or dashboard
gate is inferred. Live samples are overlapping tails, not full contiguous
logs. Final result retrieval is required before definitive analysis.


Source198 frozen376files; build,Clippy/fmt pass. Its low-rate pool-work and
regtest audits198 pass their stated scopes, and both Core198 candidates
pass height0→1 and restart1. Whole-run count1318 remains below cap10 over
307616ms; no per-hash timing claim. Pool correlation still leaves49 explicit
unmatched computations.
Fulltests198 FAIL in3 coordinator queue tests under concurrent process load.
The retained failures report missing startup marker, read timeout and missing
expected lost-reply error. A serial run of the same12queue tests PASS.
Source199 adds a fixture lifetime mutex because these real process tests share
mapped-user process accounting. Existing deadlines, assertions and within-test
worker concurrency remain unchanged. Default queue199 invocation PASS12tests.

SSH realtime189 FAILS. Matching189 failure extractor saved board-result, but
the complete process result is absent: coordinator hit a30s SSH read deadline.
Original live trace also shows backend188 PIC unsupported0x155, already fixed
in backend192. Selected dashboard run ID is unchanged; its initial HTTP408
was a failure and browser/TLS tests did not run. Its pending remote process
cannot be relabeled complete. Local189 remains active/stopped at a different
PIC FlashRange error0x5; require final retained traces before assigning cause.

Source199 is also implementing bounded streaming-SSH read-only reconnect.
Only interrupted read operations may retry once with identical bytes and
selected run. Mutations, malformed replies and worker errors are not retried.
The policy is part of transport identity. Component tests use real child
pipes; actual SSH integration still required. Next: finish full199checks,
exercise actual reconnect in the isolated network suite, then prepare a fresh
local/SSH board pair with backend192 and selected final helpers.


Source199 frozen376files. Full527Rusttests/96binaries,Clippy/fmt and release
workspace binaries/examples PASS. Local realtime189 now FAILS and matching189
extractor retained its complete board-result/guest.log/backend.log/result.json.
Last PIC55 bytes:55 AA01 at3156350939479ns, then interrupted payload55 AA at
3243274722959/3265464191069ns. The loader interprets pointer55aa and rejects
FlashRange. Backend192 application-parser fixes do not establish this missing
factory-loader behavior. Do not silently mask the address or invent a reply.

Actual SSH reconnect199 fixture FAILS its identity assertion after a real
reconnect. Client log records "reconnect once ... SSH channel closed". Review
found that it compared whole Probe values, including transient doctor checks.
Source200 uses the existing canonical WorkerExecutor::identity and saves both
identities before asserting equality. The host verifier requires both. The
failed199VM remains retained. New200fullbuild/checks active.
The199readiness policy also retains/retries an initial HTTP408 GET within the
existing bound. It still requires complete200 and exact archived body before
readiness, and does not grant a dashboard pass on408.
Next remote run should test fixed instruction clock6 with backend192 and the
4800wall/900pool profile, one VM at a time after builds finish. Prioricount171
received a real accepted first-job share but exhausted its shorter pool window.
Real-time189 showed long firmware/PIC interruption, not a valid loader profile.


Source200 frozen376files,527tests/96binaries,Clippy/fmt PASS. Actual isolated
SSH reconnect200 PASS and matching host report PASS. The complete VM
boots, tests its separate client/worker namespaces, authenticated TLS and SSH
fixtures, forced one-reply SSH channel closure, exactlyone read reconnect,
same canonical worker identity, artifact transfer/status/logs/cancellation/
retrieval/cleanup, then powersoff. Wall41583ms. This remains transport-only,
not a native dashboard, firmware or external physical host pass.

New prepared-board-icount-200 ProcessSpec
a40ed4f95428b59043ebc8d8e2cf84930168d6db6279f9acb8a8937b2823cf74/10317.
Helpers200,backend192,baselineSD004,console133,native-driver024browserpayload,
cap1000/filter1,pool-31/-30,persistentpipepool900,icountshift6,nested4800wall.
Same prepared objects are packaged in board-local-icount-driver-200 and
board-ssh-icount-driver-200. Only localVM is active,session89841,output
network-vm/board-local-icount-200. SSH is prepared but NOT STARTED.
Run it next after local completes to reduce competition for CPU/resources.
Both need matching frozen200reporter, complete selected browser/TLS checks,
fresh-work audits and Core. Previous189failures remain failures.


07:22 UTC22October: Source201 adds actual monotonic timestamps to the
shared BM1387 hash grants. Each computed event carries the globally ordered
grant and time sampled at admission, before SHA-256d. The independent auditor
checks every interval, contiguous unique grants across chains and the process
wall bound. Old traces remain explicitly unavailable for this new check.
Component tests pass; full checks, freeze and backend build are in progress.
New C ABI has40-byte ASIC events; C checks and native firmware replay are
required before acceptance. This is not physical rate calibration.

README now records the actual SSH200 transport commands. Links201 pass.
Agents remain quota-blocked until25October08:48; no independent-review pass.
Local board200 selected ID is
local-286ddd73088a411c63417cdcf98b6947bce33ae9aaaeee5b3e9eeeeaa3b70691.
It is still active. Last provisional guest tail reached SD overlay mount at
95.995virtual seconds. Its ordinary boot errors remain retained. No new
mining or dashboard pass is inferred.

A distinct BM1397 acquisition attempt used the pinned GitHub tree API and
commit-specific registers.md/diagram paths. The provider denied the API URL
and returned cache misses for the files. Exact core-mailbox field evidence
remains unavailable. Do not copy another ASIC generation's field layout.

Source201 frozen376files and final source matches its manifest. Full528Rust
tests/96binaries, Clippy, formatting, release bins/examples and470local links
PASS. The updated40-byte C ABI passes GCC warnings-as-errors plus ASAN/UBSAN;
LeakSanitizer remains disabled because the enclosing environment prevents it.
Compiled QEMU backend201 is building. Actual timed-grant firmware evidence
is still required. Active local board200 keeps its frozen200helpers/192backend.

The retained hash-audit200 of low-rate194 covers all68,877issuedjobs and1318
computations. Of49unmatched computations,25have zero midstates and tail
ffffffff00000000ffffffff, the shape of archived open-core work. Another24
have tail00000000e6e6435ed41a1217 across all3chains in four roughly50s-spaced
windows. Their full Bitcoin prefixes and origin remain unresolved. No pool
submission used these unmatched computations. Do not classify them as pool
work or add them to the12verified firmware submissions.

Backend201 UBSAN builds and all13startup checks pass. Exact binary
fe2c54a3b2d1e967c9c5fec3bea2ff823dbe498ceea17bc9d5ad3900a0139415/19937912.
Compiled bm1387-burst and PIC qtests201 pass; complete evidence is in their
qemu-s9/*-qtest-201 directories. New actual archived-firmware run
qemu-s9/native-low-rate10-grants-201 has started with helpers/backend201,
cap10/filter1/pool-31,-30/300s,360wall,baselineSD004,console133. It must pass
monotonic grant audit, fresh-work independent replay and both Core candidates.
No result yet.

Source202 adds a separate `s9_asic_audit rates RUN NEW_REPORT` gate. It
requires complete monotonic grant evidence and fails on old or truncated
traces. Hash-only audits retain their explicitly limited legacy behavior.
New regression exercises the actual trace parser with and without timing,
and with a truncated result. Scoped tests/Clippy/formatting pass; full checks
and release build are active. Backend201/model201 stay unchanged. The active
native201 run is already emitting real monotonic grant records; final results
and all mining/Core checks are pending.

07:41 UTC22October: Native low-rate10-grants201 PASS within its stated
reference scope: exit0,307130ms,full trace. Rate-audit202 checks1331actual
computations, contiguous shared grants, chain counts444/444/443, shortest
interval114487347ns against100000000ns required at10H/s. Old194 rate command
fails as required because timestamps are absent; no output pass is written.
Regtest-audit202-session0 passes12submissions, twofreshjobs, clean transition
and difficulty change. Both Core202 candidates pass genesis0→height1 and
restart1. Outputs: bitcoin-core/lowrate-grants201-audit202-job{0,1}.
Nine additional native binary-protocol probe sessions remain incomplete.

Pool-work-audit202 correlates1284of1331computations against advertised jobs,
from69598issuedjobs and1309computedstates.47computations remain unmatched;
all_computed_hashes_correlated=false. This diagnostic does not weaken the
submission verifier. Source202 full528tests/96binaries,Clippy/fmt,release
bins/examples andlinks PASS. README has now been updated to the actual201
run and202rate command; its last text change passes link checks and is the
only change since the202source freeze. Original-board and exact-target gates
remain open; this is adapted S9 reference firmware execution.

Source203 is frozen. It forwards explicit rate evidence into mining reports
and makes shared native restart suites require the rates command before Core.
No device model change from201. Full203checks/release build are active; next
run is automated-nand-cycle-203 with backend201. Author review and exact
source identities: reviews/hash-grants-203.md and -inputs.json. Independent
review remains quota-blocked. Active local board200 last provisional native
API sample30 reports connection refused while procd lists bosminerPID2085.
No native mining or dashboard pass is inferred from that service state.


08:12 UTC22October: Automated NAND203 completes all three phases, exit0,
full traces: write248299ms, read-erase249427ms, erased-readback250593ms.
Each phase passes paired storage provenance, clean independent SD inspection,
actual rate grants, two fresh jobs, clean transition, difficulty change and
both Core block acceptance/restart tests. Rates:78801/118668/95576computed
hashes at1000H/s maximum, minimum intervals1000000/1000038/1000000ns.
Full outputs: qemu-s9/automated-nand-cycle-203/assessment.json, progress.json,
run-{0,1,2}/{rate-audit,regtest-audit}.json. Six Core results are linked there.
This remains generated reference NAND in an adapted original-firmware path;
no physical power-loss, calibration, original-board or exact-target gate.
README now uses the actual tested203command and201backend.

Source204 extends actual SSH reconnect coverage to a selected active run.
After the coordinator observes both real log streams, a one-reply SSH relay
forces channel closure across Status, stdout Logs, stderr Logs, Status. Each
read retains the selected run ID; exactly three reconnects are required.
Cancellation, final verified byte match and acknowledged cleanup follow.
The host reporter now checks this previously unchecked coordinator event.
A shared bounded one-reply helper allows only read requests in the private VM.
Initial compile found a misplaced log-path argument; fixed before execution.
Scoped tests pass. Full tests/lints and the actual isolated network test204
are active. Release204 and source freeze204 complete. No pass inferred yet.


08:16 UTC22October: SSH-live204 actual isolated network test and matching
host report PASS. Selected run e0a1a2bacc6093d787cdfda0b350b457e1bfaa13667bd055b3ec90bc59d537fc
stays active across four read requests and three forced SSH reconnects. Both
real streams match the final cancelled result; acknowledged cleanup succeeds.
This remains a shell transport fixture inside separate namespaces in a local
VM, not firmware, native dashboard or an external physical host. Report:
network-vm/ssh-live-reconnect-204/remote-report.json. All530Rusttests/96binaries,
Clippy/fmt/release and471guide links PASS. Source204 has377frozen files.

BM1397 alternate primary-source research found GPTechinno/bm1397-protocol,
published crate0.2.0 and current src/{register,core_register}.rs. The provider
can list the repository tree, but raw/blob/docs source fetches fail. No core
mailbox semantics were inferred from other chip generations. Saved provider
responses and exact hashes: bitaxe/bm1397-primary-search-204/manifest.json.
Acquire the pinned upstream archive next when an authorized route is available.


08:51 UTC22October: Source205 adds provisional selected dashboard diagnostics
(attempt count, last real HTTP outcome, error, selected ID) to30s board live
samples. Native bounds remain unchanged in205. All530tests/96binaries,
Clippy/fmt/release/links pass;377sourcefiles frozen. README uses actual204SSH.

Local board200 COMPLETE FAILED. Matching200 failure extraction retained the
full board result in network-vm/board-local-icount-200/board-result. Outer VM
exit0/full4821199ms; inner firmware wrapper exit1/full4734581ms, QEMU SIGKILL,
error guest serial closed during service verification. Guest reaches465.90s
at end of console; final ASIC trace virtual658.752927135s. Actual original
uhttpd listens8000; port80 readiness fails300attempts:207empty replies(curl52)
and93timeouts(curl28), allHTTP000. Dashboard error is the fixed300-attempt
bound. Two pool sessions0/4 report completed with19/18submissions; all six
session records retained. Full mining audit200 rejects the failed execution;
no Core acceptance or mining pass is claimed. SSH200 was NOT STARTED because
this common readiness bound is known to fail. Next: derive readiness count
from the selected process budget, then diagnose native port80 service timing.
Do not describe longer waiting as a native dashboard pass.

Source206 adds intentional emulator interruption and snapshot schema4.
SerialGuest requires owned live child, actualSIGKILL status and complete serial
EOF. No final guest sync/reboot is requested. Snapshot pairs require the same
interruption receipt AND exact actual parent execution/exports. Native schema3
remains separate. Unit tests use real owned shell children and reject prior
exit, other signal, changed bytes and native restart text. Pair mutation tests
reject changed interruption provenance. New s9_nand_cycle optional final arg
controlled-interruption omits the first-phase sync and read-only checkpoint,
then restores both media for native read/erase/restart phases. This is software
interruption after completed native writes, not physical power-loss atomicity.
Full531tests/96binaries,Clippy/fmt/release PASS;377files frozen206. Backend201
remains the explicitly selected unchanged model build.

Actual interrupted-nand-cycle-206 run0 exited0/full238145ms and retained both
schema4snapshots with the actualSIGKILL receipt. Cycle FAILED at disk-write:
strict e2fsck after journal-only replay returned0 but reported free-block and
free-inode count mismatches. The parser correctly rejected these diagnostics.
No recovery phase started. Preserve disk-write/result.json and all tool output.
Do not weaken the clean-filesystem assertion. Next: retain explicit damaged
pre-recovery observations, boot the unchanged firmware from the original paired
snapshots, then require strict clean inspection and NAND readback after recovery.
Independent run0rates/fresh-work/Core manual206 checks are active session95916.
No result is claimed yet.


08:59 UTC22October: Source206 run0manual rate/fresh-work audit and both Core
acceptance/restart tests PASS. Paths: interrupted-nand-cycle-206/run-0/
{rate-audit-manual206,regtest-audit-manual206}.json and bitcoin-core/
interruption206-job{0,1}. The failed automated cycle remains failed.
Manual recovery uses the original interrupted206paired snapshots and the exact
captured console-read-erase.sh. interrupted-nand-recovery-206/run-1 exits0,
full249370ms. Native readback/erase commands finish, native reboot and paired
schema3export pass. disk-1 strict inspection exits0 without journal replay.
Its fresh-work/rate/Core206 checks are active session9921.

Source207 adds explicit observe-recovery SD inspection. It retains known
unrepaired free-block/inode counter mismatches as defects, not clean passes.
It rejects other diagnostics, malformed/repeated values and accepted repairs.
Default strict inspection is unchanged. The automated interruption cycle uses
this pre-recovery observation only on its first interrupted snapshot, then
requires unchanged strict inspection after both native recovery/reboot phases.
The signed-update cycle retains its existing strict journal-replay behavior.

207also fixes the proved300attempt readiness defect: count derives from the
selected process wall budget (481attempts at4800seconds, ten-second cadence).
HTTP attempt timeout remains3s and readiness still requires complete200 plus
exact archived-body hash. The host verifier binds the policy to the selected
process. This fixes premature probe exhaustion, not the native service failure.
Full207tests/lints are active session93237. Release207 and377file source freeze
are complete. First207observe-recovery of failed206snapshot and then a fresh
full interrupted-nand-cycle-207 were started. No automatic cycle pass yet.


09:05 UTC22October: All532Rusttests/96binaries,Clippy/fmt/release/links207
PASS. Current source matches all377frozen207files. Author review:
reviews/interruption-207.md and -inputs.json. Independent review remains open.
Actual207pre-recovery observation of206disk retains clean=false and both
free-count mismatches; original strict failure is unchanged. New complete
interrupted-nand-cycle-207 is active session62776; its run0 exits0/full238222ms.
The driver is checking phase0 and will continue the two native recovery phases.
No completed cycle result yet. Manual recovery206run1 rates, fresh-work and
both Core acceptance/restart tests PASS; outputs bitcoin-core/
interruption206-recovery1-job{0,1}. Its run1disk also passed strict inspection.

Potential next dashboard work, NOT IMPLEMENTED: the inner4800s profile sets
per-process CPU4790s although QEMU has2guestCPUs plus support threads. Local200
retained cumulative waited-child CPU4962s beforeSIGKILL; the exact cause still
needs verification. Consider a separately named bounded2CPU resource profile
with the SAME4800wall/guest clocks and a distinct frozen run identity. Do not
silently enlarge legacy limits or lower assertions. The known premature300
readiness-count bound is fixed207, but no new native local/SSH run has tested it.
SSH200 remains prepared, never started; do not repeat it unchanged. Full native
port80 startup and complete process execution remain unresolved. Its native
uhttpd8000 listener alone does not establish Boser port80 readiness.


09:27 UTC22October: Full interrupted-nand-cycle-207 PASS. All three real
boots exit0/full:238222,249207,250220ms. The interrupted first disk remains
clean=false with both free-count defects recorded. Later native recovery
and erased-readback disks pass unchanged strict inspection without replay.
All three rate/fresh-work audits and six independent Core acceptance/restart
tests pass. progress.json links each result and block hash. README now gives
the exact tested207command. Scope remains software interruption after completed
native writes, not atomic mid-write or physical power loss. Strict207negative
inspection of the damaged206disk also PASS as a rejection: exit1 and no
inspection.json. Its log is recovery207-strict-negative.log.

Source208 records exact owned-emulator wait4 usage separately from cumulative
waited-child accounting. It caches the reaped status, never reaps unrelated
children, and avoids killing an already reaped PID. Real shell regressions
check exit7, SIGKILL, repeated reads, and an independently exiting sibling.
The explicit nested-tcg-4800-two-cpu profile keeps wall4800 but permits CPU9590.
Legacy4800profile stays CPU4790. This tests a possible resource cause, not a
proved explanation of local200SIGKILL. CLI requires nested-tcg,budget4800 and
both guest CPUs; preparation and confinement enforce the same named profile.
All533Rusttests/96binaries,Clippy/fmt/release and guide links PASS. No C added.
377sources match core-checkpoint-208/manifest.json. Binaries are copied from
the completed release build into ssh-target-208; binaries.json pins all58.

Prepared-board-icount-208 selects backend201 with timed hash grants, original
SD bundle006 and saved baseline004, console133, monitor031, cap1000/filter1,
pool-31/-30,900s persistent pool,icount6. New4800wall/9590CPU profile and208
readiness policy are explicit. Local/SSH packages share these exact inputs.
LocalVM network-vm/board-local-icount-208 is active session54903. SSH208 is
prepared but NOT STARTED; wait for local readiness outcome first. All three
agents remain service-quota blocked until25October08:48AM. Independent review
remains open. Do not retry unchanged agent or source-fetch failures.


09:42 UTC22October: SSH209 artifact acknowledgement-loss test PASS with
matching frozen209host reporter. The test-only relay forwards actual worker
Begin/Chunk/Finish requests, then withholds the selected successful reply.
The client records the interrupted SSH operation; no mutation auto-reconnect
occurs. It queries the destination and resumes via production transfer_object.
Fresh270336-byte chunk case e3e6344142fd96a984451ecfbb377b0460d2ca4ab5c38c714ad11b23c4d2f78e
resumes at262144. Fresh publication case9a69c37a5e7323c18b03569be93d637bf75a7daf463a70ecd97e0ccde27d33c0
resumes at270336. Complete retrieved bytes match. Both worker identities and
final destination verification match. Existing live SSH reconnect, cancellation,
cleanup and TLS component checks also pass. This is one disconnected localVM
with separate client/worker namespaces, not physical remote hardware or firmware.
Report: network-vm/artifact-ack-loss-209/remote-report.json.

Source209 has379files,535passing tests/96binaries,Clippy/fmt/release/474links PASS.
Initial compile errors (private protocol constant and OsString comparison) were
fixed before all checks and execution; logs remain. Author review and inputs:
reviews/transport-209.md, reviews/transport-209-inputs.json. README added actual
209commands afterward. Source210 is that documentation-only checkpoint.
Clean offline build/test with empty clean-cargo-home-210 and new clean-target-210
is active session71148. Do not claim its result yet. Binaries209 remain frozen.

Local board208 is still active session54903, same selected ID
local-99ad6274b30bf175b6f4aea0ee6fc5d65e2a48d14a8a15419c32f7859d302eb3.
Latest observed client elapsed661392ms,65HTTP attempts, no readiness pass.
Read live data from its guest.log, parse only MD_SIM_BOARD_LIVE and whitelist
fields. The launcher log stays empty until completion. Match the208reporter
to its eventual result;209requires a new artifact test absent from208.
SSH208 remains prepared, not started. New BM1397versioned source URLs also
failed. Responses/hashes: bitaxe/bm1397-primary-search-208/manifest.json.
Pinned ESP/Gekko code confirms0x3c=0x80008074 initialization bytes but does
not explain the mailbox bits. No unknown semantics were invented.


09:47 UTC22October: Clean210 offline build and all535tests/96binaries PASS.
CARGO_HOME=work/simulator/clean-cargo-home-210 was empty at start; target
clean-target-210 was new. Installed Rust/Cargo1.94.1 is the declared toolchain.
All58 built release executables match frozen209 byte for byte. The actual
clean CLI froze and verified3611source/vendor inputs. Reports:
reports/clean-build-210.json and reports/source-210.json. This is a source
build check, not clean installation of all firmware targets. Source210 is
documentation-only after209; code209 tests and binaries remain identical.
Local208 still active session54903; no native dashboard pass yet.

Possible next independent implementation, NOT STARTED: the S9IO docs still
identify missing Zynq fabric-clock/reset connections. Pinned QEMU SLCR files
and original CPU timer patch exist in qemu-s9/cpu-timer-derived-001. The Rust
S9State currently advances elapsed_ns by revision.core_period_ns(5 or20),
while C md-sim-s9io.c uses fixed core_ns. A correct clock connection needs
fractional periods and separate ASIC time while the fabric is gated. Do not
just change C's period and leave Rust/ASIC time inconsistent. Verify the
FPGA clock/reset wiring from pinned board source and kernel clock driver
before design. No new code or assumption has been committed for this work.


10:17 UTC22October: TTC clock regression211 reproduces the backend201 defect.
The pinned DTB selects clkc6 (cpu_1x) for both TTC blocks. Kernel source
5aa5c0915c93c6078984b13367edd12670e8ad25d922d352654f768d7005db8f
and QEMU source2a40e6705e59fd8807012631bc6b2c0877f10481b12768f78e4f9d7b88376cce
were inspected in isolated runs clock-source-211/kernel-clock-source-211.
TTC baseline211 has328 responses/68differences. Actual133ticks instead of111;
the old fixed clock also keeps counting while the SLCR branch is gated.
New C patch s9-ttc-clock.patch connects cpu_1x, retains fractional periods
and stops disabled/gated/external-unconnected counters. It is UNDER TEST.
All535Rust tests/96binaries,Clippy/fmt212 PASS. No backend success claimed.
Backend212 stopped before compile at the strict BM1387 parent-board hash.
Composition213 records the unchanged original BM1387 parent/derived sources
plus TTC wiring. New exact whole-file hashes replace both assertions; neither
assertion was removed. First freeze213 invocation had an extra repo argument
and failed before creating output; corrected invocation is active session18458,
with backend-build-213-ubsan as the new isolated build. Log ttc-clock213-backend.log.
Native local208 remains active session54903, selected ID unchanged. Latest
readiness sample290attempts/2916463ms, HTTP000 empty reply. No readiness pass.
SSH208 remains unstarted. Agents remain quota blocked; independent review open.


10:37 UTC22October: Backend213 UBSan build PASS, binary
9de4448f6a9d10b98813a9e14e5cd7b5e4fc9eb8192e7ff43f4ca78d9b3e1b98/19946872.
All13 startup checks pass. Compiled TTC clock oracle passes328responses;
PIC and BM1387 regressions pass. Probe213 had stale fixed133MHz scope text,
corrected in214 without changing its oracle. Actual214rerun also passes328.
README links the evidence. core-checkpoint-214 captures the scope/docs change.
Author review: reviews/ttc-clock-214.md. Independent review remains quota blocked.
Full Rust535tests/96binaries212 and release/Clippy/fmt213 PASS. Scoped214
probe build, Clippy/fmt and links PASS. Original213helper binaries frozen in
ssh-target-213 (58). interrupted-nand-cycle-213 active session38599; phases
0 and1 have passed rate/fresh-work audits and four independent Core blocks.
Final erased-readback boot is running; no full-cycle pass yet.

Review found no whole-device TTC reset handler in the upstream model. New
qemu_ttc_reset Rust example issues actual QMP system_reset and inspects all
six TTC channels. Attempts215 (Unix socket bind) and216 (inherited socket
validation) failed at environmental socket restrictions, before device tests.
Attempt217 uses private run-owned FIFOs and QMP stdio. It reproduces the
defect: first post-reset count-control read0 instead of cold default0x21.
The QMP RESET event is real and retained. No assertions weakened.
Separate s9-ttc-reset.patch restores original cold defaults, clears fractions,
cancels callbacks and lowersIRQ. Exact parents/patch in ttc-reset-derived-218.
Build218 active session22667, backend-build-218-ubsan; frozen core-checkpoint-218.
RustClippy/fmt/fulltests/links218 active session56534. Do not claim new reset
model passes before compiled reset oracle. Current probe uses two actual
resets, checks384transactions and requires clean emulator exit.

Local board208 still active session54903. Latest observed4181892ms/416HTTP
attempts; empty reply, no native readiness. Same selectedID throughout.
SSH208 is prepared but unstarted. New console-service-diagnostic-219.sh
adds read-only service/thread/wchan/listener/entropy details to existing
sample points. It is not executed yet. Its separateJSON pins parent133
and derived610c26e6f60aa5be77a4f178b750236fd5c9f7ad6246a50b3b7f73d75e6bc5eb.
It adds no service start, firmware patch, fabricated status or clock change.
The next changed network run should use these observations to locate the
missing nativeHTTP service; await208outcome first.

BM1397source research214 recovered provider-rendered registers/core-registers
text and a GitHub SVG-link commit lead6798f0512cc74bbcf138077c6b41dbc083577ea8.
ExactSVG/source bytes remain inaccessible. GPTechinno bm13xx-rs is a further
primary source lead; subpaths mostly cache-miss. Allresponses/hashes retained
in bitaxe/bm1397-primary-search-214. No mailbox semantics inferred or changed.


10:44 UTC22October: interrupted-nand-cycle-213 PASS. Three boots exit0 at
238172,249734,250483ms. Allrate/freshwork audits, later strictdisk checks and
six separate Core acceptance/restart tests pass. Blocks and input hashes
are linked by progress.json. This remains adaptedreference software
interruption after completedwrites. README now gives the exact213command.

Backend218 UBSan build PASS, binary
b530f76bbdf5ba9a3250b87683ccf8cd3322a01579dda9f5e1a8a94fc469d425/19947032.
All13startupchecks,384resettransactions with2actualQMPresets,328clock
transactions,PIC andASIC compiledregressions PASS. Rust537tests/97bins,
Clippy,fmt,release and478links PASS. Source218 and59releasebinaries frozen.
Authorreview reviews/ttc-reset-218.md; independentreview remains unavailable.
The selected218signed-update/nativebrowsercycle is active session38057:
qemu-s9/automated-update-cycle-218. Do not claim result before assessment.

Prepared-board-icount-219 uses backend218 and the read-only service
diagnostic console219, retaining4800wall/9590CPU and the prior cap/filter/
pool,icount6 and saved004baseline. First networkpackage219 failed because
the209template has a null payload_manifest field; requirement expects absence.
Corrected package board-local-icount-driver-220 uses the plain namespaces001
template and frozen218helpers. It is PREPARED, NOT EXECUTED. Awaitlocal208
(session54903) outcome first. Match208reporter to208,218reporter to220.

11:01 UTC22October: local208 finished. Outer dashboard test FAILED; no native
readiness after471HTTP attempts. Inner driver exit0/wall4733886ms, no truncated
streams. Owned emulator usage5019.749359CPU seconds, below9590ceiling. This
does not prove the cause of the earlier200failure. All208sessions retained.
Matching208failure extractor succeeded; strict208report rejects failed outer
prerequisite. No failure verdict was changed. SSH208 remains unstarted.

New Rust s9_retain_evidence generates missing observations from locked retained
execution. It validates inputs, selected identity, tools, envelope and streams,
copies to a new directory and rechecks all hashes. Actual negative CLI tests
reject changed inputs/identity/stream before creating output. Rust553tests/
98binaries,Clippy/fmt PASS. Frozen executable evidence-driver-221. Authorreview
reviews/evidence-retention-221.md; independentreview still unavailable.
mining-evidence-221 and retained-replay-221 preserve originals and verdicts.
Unchanged208auditors pass rateaudit and both completed sessions0and3. Four
separate Core acceptance/restart checks PASS: core-session0-block0-221,
core-session0-block1-221,core-session3-block0-221,core-session3-block1-221.
mining-assessment-221.json links results and retains dashboard_passed=false.
These are adapted reference results. Exactboard/fullmatrix gates remain open.

Changed networkrun220 ACTIVE session66285 since10:47, selectedID
local-317760d3fd2300627a301d32f2e8e7efaeed35e734141df832946779d8ff76e7.
Uses prepared219, backend218, frozen218helpers and read-only console219.
Latest50HTTP attempts/510901ms; no readiness. Guestprocd reachesubus109virtuals.
No diagnostic service sample yet. Match218reporter, not208.
Signed-update/native-browsercycle218 ACTIVE session38057. Provision andrun0
complete; nativeHTTP/configwrite/install/rate/mining checks PASS onrun0.
Two Coreblocks retained. Browser is tested inrun1, notrun0. No completecycle
pass yet. Continue actualbrowser/update/restart checks and diagnose220services.

11:16 UTC22October: signed-update/nativebrowsercycle218 COMPLETE PASS.
Provision pluswrite/restore/verify boots exit0; phasewalls307880/366254/249512ms.
Native browser observation passedtrue/failuresempty. Tests realassets/login,
native mining view/WebSocket/cookie isolation. Strictdiskchecks/rate/freshwork
allPASS. Eight separate Core acceptance/restart checks PASS. assessment.json
still reports original_board_gate and full_native_dashboard_gate incomplete.
README gives actual frozen218command. This result is not reused for laterbackend.

Source221 frozen, guide links PASS. New BM1397research222 confirms published
bm1397-protocol0.2.0 package exists; all exactsourceprovider URLs failed. Saved
responses/manifest in bitaxe/bm1397-primary-search-222. No mailbox modelchange.

TTC review found match-enable ignored and bit6 discarded in both controls.
Primary AMD UG585/vendor-driver responses saved in ttc-mode-reference-222.
New184transaction baseline222 has24differences onbackend218. SeparateCpatch
s9-ttc-modes.patch, strictparent/output hashes and source223checkpoint.
backend223 UBSan PASS, binary9930e9a8c77cb435cb186f2fbd04e5dad99889db54d908e56889f50e9ad03648.
Compiled modes223184/clock223328 tests PASS. Stronger qemu_ttc_reset224 probe
checks385transactions and24realGICinput changes across2QMPresets. Actual224
tests PASS on218and223. Allsix IRQwires raisebefore/lowerat eachreset.
Full Rust553tests/98bins,strictClippy/fmt223and224PASS. Source224 hasstronger
resetprobe after223snapshot; no earlier result reused as newprobeexecution.

Further review found downcounterreset loads0 instead of65535/intervaltop.
Baseline225 has250transactions/30differences on223. Exactbaselineprobe and
oracle retained andhashchecked. Newseparate s9-ttc-decrement.patch UNDERTEST:
complementary fractionalphase for downcounting, matchpoint onsameclockedge,
directionchanges retainintegercount/phase. Expanded regression322transactions
includes directionchanges. source226/backend226 ACTIVE session3792. Do not
claim pass until compiledoracle. Re-run clock/modes/reset andfirmware afterfix.

Local220 remains ACTIVEsession66285. Latest149HTTPattempts/1502699ms no native
dashboard. Onlyinitialstartupwait observed; native service diagnostics pending.
Match218reporter tothisrun. SSH208unstarted. Independentreviewagents stillquota
blocked; no fullrelease or exacthardware gate closed. Next: finish226tests,
freezehelperbinaries, diagnose220actualservice state, runaffectedfirmwarechecks.

11:29 UTC22October: backend226 UBSan build PASS, binary
14c33c658e0bd1286bb69982b500b9468d0e2ed7c8f3f33d1aea4111050ccbf8/19947128.
All13startupchecks,322downcounter/184mode/328clock transactions,385reset
transactions with24actualGICevents, PIC andBM1387 regressions PASS. Stronger
resetoracle224 has the same reference-device scope asabove.
Authorreview reviews/ttc-modes-decrement-226.md. Release/Clippy/fmt PASS.

Fulltests226FAILEDtwice at isolated_worker.rs140 fixturewrite, EDQUOT shared/tmp.
Unchangedisolatedtest passesalone. Same11testbinarypassesinparallel usingprivate
TMPDIR=work/simulator/test-tmp-227. Fullworkspace rerun227 withthatTMPDIR PASS:
553tests/98bins. No assertions,fixturesize,limits orselection changed. Shared/tmp
failures retained. Removedonly238disposable rlib/rmeta intermediates from
completedclean-target-210,317846353bytes; reports/executables/archivespreserved.
Cleanupmanifest reports/intermediate-cleanup-226.json. 489guidelinks/fmtPASS.

Firsthelpercopy226 incorrectly skipped3names oflength16 without a dash, so
automated-update-cycle-226 FAILED beforefirmwareexecution (missing s9_update_bundle).
Do not repairthatfrozenattempt. Correctedcopy ssh-target-227 has60executables
withbinaryhashmanifest. Samecompiled226code. Freshsignedupdate/browsercycle227
ACTIVE session38354 onbackend226. Log update-cycle227.log. No completedpassyet.
Frozen227downcounter/resetcommandreruns ACTIVE session6228. These onlyverify
documentedcommandpaths.

Local220 ACTIVE66285: observed212HTTPattempts/2134089ms, no readiness. Newconsole
proves boser2072 ISrunning; S99boserlink exists, entropy87then141. Boser single
thread staysR, userticks1092->2405->8032 andRSS399->802->3006pages. SavedPCs
b6fa0f88,b6fa12d4,b6fa1414,b6fa1508. Thisisprogress, notproof ofdeadlock orservice
readiness. Needmaps/PCandnative service logs ifnextdiagnostic is needed. No
unnecessary newVMstarted. SSH208stillunstarted. Usematching218reporterfor220.

11:42 UTC22October checkpoint: inspector229 now captures watchdog sources. Both
isolated QEMU/kernel inspections passed. Kernel cadence_wdt.c SHA256
997a88046439043abd6609d9c3be1b68de6eaf57b56a5f5c5d564e9bb633c75f.
The original device tree enables SWDT at f8005000, IRQ41, clkc45. Pinned QEMU
has an unimplemented device at that address. Existing PTIMER will be reused.
AMD leaf URLs with actual TOC contentId links work; exact fields and functional
contract saved under watchdog-reference-229 and watchdog-reference-231.
The counter runs regardless of WDEN, stops at zero, and status clears on restart.
Output enable and pulse widths must be tested separately. No watchdog model
has passed yet. Original-board and full-matrix gates remain open.
Frozen227 downcounter/reset command reruns228 passed. Full553 tests227 passed
before the inspector-only229 change. New console-service-maps-228 script is
prepared but unexecuted. Active220 and227 sessions remain66285 and38354.
Agents still have the recorded quota failure until25October08:48.

Watchdog231 implementation under test: Rust registers and C QEMU PTIMER adapter,
SLCR clock connection, explicit reference limitations in SWDT.md. Three Rust
register tests pass (all4096mode/control keys and65536restart keys checked).
The first literal reload expected decimal had an arithmetic error; corrected
from1191935 to0x123fff=1196031 for CRV0x123. Initial failing log retained.
Primary fields and exact contract saved with hashes. Hidden power-on count,
external APB hang, BootROM reset reason and external reset waves remain open.
The model does not claim those as implemented. Baseline231 compiled probe and
UBSan backend231 build started; no compiled watchdog pass yet.

12:00 UTC22October: signed update/browser cycle227 COMPLETE PASS. Fourboots,
eight independent Coreaccept/restart results, native settings write/restore/
persistence and browser checks pass onbackend226. Originalboard/fullnative
releasegates remain incomplete in assessment.result. Noexactmatrixcaseclosed.

SWDT232 UBSan backend COMPLETE PASS; binary884967ee15ccc44094663f527a276d07d2059e61440416b011b4591aa26d1671/19952952.
Baseline231failed118actualvs126expectedresponses, noIRQ. Build231 rejectedwrong
SLCRparentbeforecompile. Correct232parent95301bc40f1bc0db82fbc7124ee6a3df5e61018923cde270dbf4f3bb758efd8c.
DerivedSLCR68a7229709ef69db7187a54ba0d5361d8edc6a20fd5c7321a2cf0936ad71e1fd.
ModelusesRustregisters/QEMUPTIMER; reset-hidden-count assumption, absentclock
APBhang, externalresetwaves, BootROMreason, migration/calibration remainopen.
Compiledswdt233/234146orderedresponsesPASS incl4divisors/pulses,CPUgate/external
clockstop/resume. Actualreset233/234PASS60transactions,3resets(includingreal
watchdogcausedreset),2IRQtransitions andcancellationofoldcallbacks. TTCclock328,
TTCreset385/24IRQ,PIC881,ASIC859regressionsPASSonsame232backend.
Frozen23360helpers+manifest; source234checkpointcapturescurrentcode/README.
Fulltests232PASS556/98. LatestClippy234/fmt/496linksPASS; fulltests234ACTIVE10241.
Authorreviewreviews/swdt-234.md. Independentagentsstillunavailable.

Newactualfirmware signedupdate/browsercycle234 ACTIVE76381 on232backend with
frozen233helpers. Exactcommand in checks/update-cycle234.log anddriverinput.
Local220 ACTIVE66285,413HTTPattempts/4151467ms, no nativeHTTPreadinessyet.
Use218reporter for220, preservefailedgateifitendswithoutdashboard. SSH208notrun.

Final234 fullworkspace PASS556tests/98binaries; Clippy/fmt/496linksPASS.
Watchdogfirmware234 remainsprovisioning, nofirmwareclaimfor232backendyet.

12:18 UTC22October: Local220 COMPLETE FAILED. Inner process timed-out at4800060ms,
no truncated streams, nativeHTTP never became ready. FinalboserPID2072 has
9066user/371system ticks,3328RSSpages,zero voluntaryswitches and savedPCin
b6fa1xxx. Port8000uhttpd andnativebosminerportslisten; selectedport80doesnot.
This suggests CPU startup work but doesnotprove its cause. Completeguestlog
andmatching218extractor retainedboard-result. Strict218reportfailedexpected.
Retainer233 copiedexactlockedstreams toretained-replay-238 andgeneratedpool
observations, thenreturnedfailureforexecutiontimeout. Ratesaudit218PASS;
regtest-session0and3audits218REJECTexecutionincomplete. NoCoreclaimfor220.

Watchdogtrace additionfirstfailedcompile236 at serde_json macrorecursionlimit.
Movedthefieldto thesmallbackend identityobject;238compilepassed. Prepared237
accidentallyusedthepreviousunchangedexecutable after236compilefailed. Itis
unexecutedandnottraced. Prepared235also unexecuted; deployment235failedbecause
helperpathlacked/release; deployment236succeededbutisunexecuted. Preserveall.
Actualprepared238 hasnewcompiledtraceenabledforverifiedSWDTbackends.

NewlocalVM238 ACTIVE91042. Prepared-board-clock0-238, backend232, mapsconsole228,
frozen233remotehelpers andcapturednewboot-runner238. Deployment
network-vm/board-local-clock0-driver-238; executionboard-local-clock0-238.
SelectedIDlocal-3267df38ff77765cec386f54a23f7e9855025a66017a8c22aae1bd75644e8b97.
Clockshift0instead6; same4800wall/9590CPU,cap1000/filter1,pool-31/-30/900s,
unchangedarchivedfirmware andsaved004baseline. Newseed/time; newmodelbackend
andreadonlymapsdiagnostics. No performanceattributionorpassassumed.
UseMATCHING233reporterfor238. SSH208andclock0SSHarenotstarted.

Source238snapshotcaptureslatestcode. Clippy238/fulltests238PASS556/98 after
tracefix. Firmwarecycle234 stillACTIVE76381; provisionandwritephasepass,
restore/browserphaseinprogress. Backend232hasrealfirmwareexecutionalready,
butcomplete234assessmentpending. BM1397primarysource235indexesfoundbutexact
codeblobsinaccessible; no mailboxmodelchange. Authorreviewonly, independent
agentsremainquota-blocked. Fullmatrixstillincomplete/zeroexacttargetpasses.

12:23 UTC22October: Preparing a second performance route without changing
firmware: non-UBSan QEMU release build239, samepinnedCPU/source/modelcode.
Prior232sanitizedcomponentregressionsremainretained. Build239 ACTIVE; no
performanceclaimuntilactualrun. Motivation: nestedTCG executes the sanitizer
checks too; productionbackendneednotcarryinstrumentationaftertestedchecks.
Every239device/firmware result mustuseitsownbinaryidentity. Clock0run238
remainsactive; at150s onlyoriginalUbootautoboot countdownobserved, noLinuxyet.

12:38 UTC 22 October: Cycle234 COMPLETE PASS on UBSan backend232. Four actual
archived-firmware boots, native configuration write/restore/restart checks,
native browser and eight independently verified Core acceptance+restart tests.
assessment.result retains original-board and full-native gates incomplete.
Provision block hashes068933ff01867c338b875346fb55d7d3e99e28558e5399546da9f3997ae44d7b
and04c975c701121e4ac2789b7a2195cf77f8020fc4dfe69cbe75cb27f2f34e429f.
Three phase pairs are in progress.json and linked Core evidence. No exact target
completion is inferred. Final formatting and496 guide links238 also passed.

Production backend239 COMPLETE PASS, no UBSan instrumentation. Binary
29ec8c89e4fb9168e4669bef7c7484f722889010652080d25b3f38c3cae1caa2/9791312.
Same pinned QEMU/model sources as tested232. Fresh compiled SWDT146, CPU/TTC328,
PIC881, ASIC859 and actualSWDTreset60/3resets tests PASS in *-release-239.
No production firmware result yet. Local240 ACTIVE77510 with production239,
clockshift6, mapsconsole228 and capturedboot-runner238. Prepared-board-release-240,
board-local-release-driver-240 and board-local-release-240 hold its full inputs.
Same4800wall/9590CPU, cap1000/filter1, pool-31/-30/900s, originalfirmware+baseline004.
SelectedIDlocal-eb7d02277b685b2c5c71e42f6cb857d75540d331d36136cca256ecccd3aad529.
Matching233 reporter required. Clock0run238 ACTIVE91042; now in actualLinuxboot.
No dashboard success in either run at this checkpoint.

BM1397 raw refs/heads/master register Markdown now accessible through provider;
SVG and exact Rust source remain cache misses. Do not repeat unchanged requests.
Extending isolated ESP source inspector with existing archive PLL members; no
external download execution. Register0x14 per-chip storage/readback/reset added,
with its exact programmed filter exposed to shared-engine conversion. This does
not bypass unsupported core command0x3c. Focused tests241 running; no firmware
pass claimed. Independent agents remain quota-blocked until25October08:48.

13:07 UTC 22 October: All557 Rust tests in98 binaries, Clippy and formatting
passed for the BM1397 filter change242.497 guide links pass. Probe035 still
exits1 at core0x3c; ticket writes are not reached. Author review is in
reviews/bm1397-filter-242.md. Isolated PLL source241 captured actual archived
pll.c and its450MHz upstream test. New provider views are saved in
bitaxe/bm1397-primary-search-240; missing SVG/source responses remain blockers.

Actual native watchdog inspection244 passed (backend239, boot-runner238).
It observed native procd watchdog API, running/timeout30/frequency5, real
/dev/watchdog nodes and actual SWDT MMIO writes. New Rust example
s9_watchdog_check and support/s9_watchdog_scenario.rs run a native fault through
ubus, read status with archived devmem, require the kernel IRQ message, resume
feeding and restore settings. No guest byte patch, synthetic success or NIC.

Fault245 FAILED its restore assertion: native API clamps frequency against the
old timeout when both are changed in one call. Restore now sets timeout30
before frequency5. The guest already demonstrated status0->1->0 and real IRQ.
Also, the actual2022kernel is4.4.0-xilinx-g518c00cd977f-dirty and prints
'Watchdog timed out. Internal reset not enabled'. The source member229 is a
reference, not proof of exact compiled-driver identity. Keep that distinction.
Unit245 failed integer inference; fixed timestamps to u64. Unit246/247 found
a marker-at-offset0 bug; fixed by checking nonoverlapping ranges. Unit248/249
pass. Fault249 uses explicit icountshift6 to observe the short IRQ pulse;
normal host-clock245 had callback latency beyond the1us test bound.

Production watchdog-fault-release-249 PASS:1327 device records,8 healthy and5
recovery pings, expiry2264887360ns after last ping,64ns IRQ pulse, status0/1/0,
restored native30s/5s settings. UBSan watchdog-fault-249 FAILED strictly:
one real expiry occurred while native feeding was enabled (2.68978s between
pings, above2.264887s hardware timeout). Guest recovered and restored, but
that does not turn it into a pass. Investigation remains. No assertion relaxed.

Localclock0run238 reached real native port80 after233 readiness attempts.
Archived JS and GraphQL requests succeeded. Its frozen233 HTTP probe then
FAILED 'native uptime did not advance': fixed3hostseconds did not advance a
whole guest second in this uncalibrated instruction-clock run. Inner mining
execution still ACTIVE91042; retain failure and later independent mining audit.
Newly started identical prepared238 SSH run250 ACTIVE91369, deployment
board-ssh-clock0-driver-250/outputboard-ssh-clock0-250. It uses frozen233 and
will retain the old strict uptime check; do not mutate its inputs midrun.

Production local240 ACTIVE77510. Readonly maps show boser savedPCb6f73408 inside
its packed image mapping (file offset2c5408), not a shared library. Original
boser has UPX markers at152,2908222,2908232. Mapping observation saved under
board-local-release-240/boser-mapping-observation-249.json. Unpacking is the
supported startup-stage hypothesis; no completion/rate inferred. NoUPXtool
available; no binary adapted. Original-clock0 path already reaches nativeHTTP.
Production signed-update/browsercycle243 ACTIVE4207; write phase passed.

Next: fix HTTP/browser uptime sampling to wait for actual advancing native
telemetry within explicit client budgets, preserving boot identity and all
assertions. A new middle instruction-clock rate can test startup and UI without
changing the old runs. Finish watchdog tests/review and run current workspace
checks. Fullmatrix/exacttargets/originalboard/independentreview still incomplete.

13:22 UTC 22 October: Watchdog fault252 passed on BOTH UBSan232 and production239
with icountshift2:147 records,7 healthy pings,5 recovery pings, status0/1/0,
expiry2264887318ns,36nsIRQ, original30s/5s restored. Frozen252 repeat253 also
passed (expiry differs by1ns). Fixed2s timeout and strict1us pulse assertion
remain. The failed UBSan clock6 run249 is retained; its actual2.689s feed gap
exceeded the2.2649s programmed hardware timer. No fake health or pass.

Review strengthened the oracle: healthy and recovered feed sequences must
span a complete observed hardware timeout; each ping gap and final status-read
gap must stay below that timeout. New fixed-vector mutation catches a missed
feed gap. Scoped254 tests pass; frozen watchdog-driver-254 holds the checker
and boot runner. Fresh stronger tests ACTIVE51166(UBSan) and56164(release),
outputs watchdog-fault-254 and watchdog-fault-release-254. Contract now states
that the captured Bitmain source is a reference, not an exact source match
for the archived Braiins kernel. This is a documentation correction only.

Production signed-update/browsercycle243 COMPLETE PASS: four original-firmware
boots and eight Core accept/restart tests on239. Assessment still retains
original-board/full-native gates incomplete. All559 tests in99 binaries,
workspace strict Clippy252 and release build252 pass. Source252 checkpoint and
ssh-target-25261executables manifest are saved. Later254 changes only strengthen
the watchdog checker; current full checks still needed after the review fix.

HTTP and browser telemetry now wait for actual guest uptime progress. Existing
HTTP120s overall deadline and20request cap remain. HTTP reserves7requests and
30s for settings+logout, with at most6 sampled queries. Browser keeps its
360s parent budget; uptime samples have a30s/16sample bound. All samples are
retained. Shared s9_telemetry validator rejects missing/bad/backward time,
changed host and changed boot identity. The browser retains its unsigned
whole-second check. Old fixed-delay failures are not relabelled.

New prepared-board-clock2-252 uses production239, frozen252 helpers, unchanged
console228/SD004/original2022image, same4800wall/9590CPU/cap1000/filter1/pool900,
fresh seed/time and explicitshift2. Local deploymentboard-local-clock2-driver-252
running asboard-local-clock2-252 ACTIVE61470. Matching252 reporter required.
SSH deploymentboard-ssh-clock2-driver-253 PREPARED BUT NOT EXECUTED yet.
Oldclock0local238 ACTIVE91042 with known uptime-check failure; oldrelease240
ACTIVE77510 with nativeGraphQLstatus000/curl28 failure. Both reached nativeHTTP.
Oldclock0SSH250 ACTIVE91369 selectedrun-93ecd6ac509d9ec176c8031961bae8ff66ee56aa1fae4f8b31a97487033367ae,
still in boot at last check. It retains old233 uptime logic. Do not mutate any
active run. After completion extract matching evidence and run strict mining
checks only when execution is complete. No final exact-target gate is closed.

13:38 UTC22 October: stronger watchdog254 tests COMPLETE PASS on both232 UBSan
and239 release,147 records each,7 healthy/5 recovered pings, >4s feed spans,
36ns virtual IRQ pulse, restored30s/5s native settings. Frozen254 binaries,
README verified command and author review swdt-native-254.md are saved.
Full current tests254 PASS559tests/99binaries; fmt254, strictClippy254 and
500documentationlinks PASS. No C/model changed after backend239.
All three independent agents remain quota-blocked untilOct25 08:48. This is
an independent-review blocker, not a passing review. Do not repeat retries.
BM1397 core research255 preserves provider responses/hash. Current public
mailbox diagrams remain inaccessible; other chip generation register maps
cannot establish BM1397 semantics. No unsupported core write bypass added.
Next independent implementation: versioned serial-wire device state with
partial-frame/queue replay and malformed-state rejection. Full board snapshot
claim stays rejected until CPUs, devices and sessions can be restored together.

14:04 UTC22 October: serial device state256 implemented and reviewed. Snapshot
version1 preserves both partial frames, queue, pins and virtual deadlines.
Restore rejects unknown versions, corrupt state and overflow. Review found
old enqueue reserved only one bit; fixed to reserve full queued transmission
and receive completion. Six serial tests pass; full256561tests/99binaries,
strictClippy256, fmt and501links PASS. Frozen25661executables/110574816bytes
and source/core256 retained. Core3eaebc005d8f7db524c82862e33cca70d7720a4edce7e66180c120444bc9658f/33450276.
New UBSan backend256e6a272d2a8b9670bb88eeb5ea430bb054d4b13b9e89c1bafc85ebb8cd8084f7d/19957048,
compiledserial-state-asic-256 PASS859responses. No fullboard snapshot claimed.
Signed-update/browsercycle256 ACTIVE77239; provision exited0,248003ms, two
Coreaccept/restartchecks; run-0 under way. This reruns real firmware afterthe
serialoverflow fix with current bounded uptime sampling.

Local238 COMPLETE FAILED. Matching233 extracted and retained255. Inner
timedout4800094ms, fulltrace. Dashboardfailsnativeuptime. Rateaudit passes
7379actualhashgrants/cap1000/minspacing1000072ns. Regtest-session0strictly
rejectsexecutionincomplete; noCoreclaim. Local240 COMPLETE FAILED dashboard
GraphQLcurl28, but innerexited0in4733939ms/fulltrace. Matching233 extracted/
retained257; independentregtestsessions0,4,5 pass twojobs/clean/diffchange.
Session0computations47matched/26408total/200serialresults. Bothsession0Core
blocks accepted+restart; sessions4/5nodechecksactive65042/28502/69353/77881.
Originaltransport/dashboardfailedverdictsremain. Noexacttargetgatepassed.

Clock2local252 ACTIVE61470, nativeHTTP PASS after111readinessattempts, browser
processfailed at~1685s. SSHclock0run250 ACTIVE91369, nativeHTTP PASS after187
attempts, browserprocessfailed. Full browserfailurelogs arrive atfinalevidence;
currentdiagnostics exposeonlygenericerror. Source257 now adds smallwhitelisted
browserlastphase/error toliveprogress, without rawpages/cookies/encodedlogs.
47remoteexampletestsPASS; strictClippy/releasebuild257PASS. Fullchecks and
newrealdeploymentstillneeded. Do not mutaterunning252/233helpers.
SSHclock2run253 started ACTIVE7962, selectedrun-7687cb519e74fcfddca8ef85d0a125850ee3b386a23f9d16ce274a75c9663f76,
matching252reporterrequired. Uses exact prepared252 and declaredisolatedVM
client/worker topology. Noexternalphysicalhostclaim.

Poolcollectionin nestedmode intentionally runs untilbudgetminus70s, even
whenbootfinishesearlier. This explains~4730s completedinner240; itis not
necessarily a hungguest. Console228 still produces debugmaps/procthreads;
cleanfutureconsole canreduceunneededdiagnosticload withidentitychange.
Next: finishnativecycle256 andselectedbrowserfailureevidence, inspectactual
lastphasebeforechangingtimingassertions; finishCore240session4/5; capture257
helpers/source beforefreshdeployment. Independentreviewstillquota-blocked.

14:18 UTC22 October: local240 all6 Core blocks PASS (sessions0/4/5,
twojobs each), accepted and persisted at height1 afterrestart. Joinedhashed
mining-assessment-258.json preservesdashboardFAIL and incompleteconnections.
Full257tests562/99PASS. All258tests562/99PASS, strictClippy258/fmt/501linksPASS.
Cycle256 FAILED atnativeupdateprecommit: LuCI initialquery omittedhostname,
requiredbynewsharedtelemetryvalidator. Actualretainedfullsampleskeephostname
andbootepoch unchangedwithadvancinguptime. Source258addsfieldwithoutweakening
assertions. Frozen25861executables/110582936bytes andcore/source258retained.
Cycle258 ACTIVE91535, provision0/248541ms +2Corechecks/diskclean; run0active.
Authorreview259foundprecommittelemetryfailurealso bypassedminerrestartpath.
Current259validatesinitialsamplebeforemutations andmovesprecommitcheckinside
validation rollbackscope. Needsnewchecks/freeze/nativecycle after258finishes.
SSH250stillactive91369 at~4192s;local252active61470 at~3429s;SSH253active7962
at~1821s nowfailsassettransferstatus200/curl28. Do notmutatefrozenrunninginputs.
WarpAxealternateprimaryreference258captured; core/simsource endpoints cachemiss.
NoBM1397semanticsinferredfromrepo listing. Manifestrecords9cbc497... rootand
BM1397profilecommitd8795a...; noexactsourcebytesavailable. No repeatedretry.

14:45 UTC22 October: cycle258 COMPLETE PASS4boots (provision/run0/run1/run2),
8Coreblocks acceptance+restart, nativebrowser/configuration/restoration anddisk
checks. Frozen258tools/backend256. This testsqueryhostnamefix but precedes259
rollbackscopefix. Source263alsochangesS9browser navigationpolicytoeager after
SSH250actualfailure showedloginformpresent/interactive whileWebDriverNavigate
normalwaitexpired90s. FinalauthenticatedcompleteDOM,exactassets,WS,telemetry,
profileisolationassertionsunchanged. Capabilitiesmustconfirmselectedpolicy.
SSH250 COMPLETE FAILEDinner4800079ms timeout/fulltrace. Matching233extractPASS,
strictreportFAIL,retainer263FAILpreservingobservations. NativeHTTPandTLSprobe
PASS inclnocertificate rejection; browserfailedNavigate90s. Local252 COMPLETE
FAILEDinner4800104ms timeout/fulltrace; matching252extractPASS. Browseralso
Navigate90s, DOMinteractivebutloginfalse. NoCoreclaimsforincompleteexecution.
SSH253stillACTIVE7962; assetHTTP200/curl28 failure (60s boundedtransfer).

NewusefulBM1397evidence: exactcitedT17archive exists! OEMandmirrorhash89677e26...
17123204bytes verified. NewRust t17_source_inspect usesisolatedtar/gzip/debugfs/
readelfandinstalledCapstone5.0.9CABI. Noacquiredprogramexecutes. t17-static259
failedconfinement128MiBfilelimitvs64policy;260failedSIGXFSZon100MiBramdisk.
Dedicatedread-onlyinspectionprofile512MiBaddress/30CPU/128MiBfilebound.261PASS,
262PASSwithA32disassembly/twoindependentliteralvectors.264READMEcargorunPASS.
Extractedbmminerad090e7abbf525a7a8a23c7153529e45af94b864e1170e677e1f85b4c4091367/553247,
ELF32littleARM,stripped.dynsym179. Originalrootfs100MiBhash735c3dc6... retained
ininspection. Debugstringsgivecore0bitfields. core-mailbox-analysis.jsonunder
262 retainsexactinstructions: readpackerVA24cb0..24cf0,write24cf0..24d30,
fileoffsetVAminus8000. Read=((core>>1)&1ff)<<16|((reg&15)<<8)|ff.
Write=80008000|((reg&15)<<8)|(val&ff). Callers24e9c/24f00send3cMSBfirst.
Staticinterpretationonly; nextactionexecuteexact128bytesunderexistingqemu-arm
withisolatedsmallCABIwrapperandindependentallinputchecks. qemu-armavailable;
clangavailable; ld.lldnotonPATHbutRusttoolchainrust-lldlikelyavailable.
Core0stringsfile72b0c..72b70:CCDLY7:6,PWTH5:4,HASH_CLKEN3,MMEN2,reserved1,SWPF0.
NoBM1397modelchangedyet; clockgeometry/resetdefault/timingunknownremain.

Full263tests566/100PASS,scopedT17tests4PASS,strictClippy263PASS,release263PASS.
Sourcescheckpoint263complete,frozen26362binaries. Productionbackend263 ACTIVE
87441 overlay239release+core263; initialwrongreleaseargumentrejectedbeforebuild,
correctcommandomitsreleaseflag. Nativecycle263stillTOSTARTafterbuild.
CurrentREADMEoutputchangedtostatic264aftercheckpoint; docs-onlychange.
Newconsole-services-clean-263.sh +jsonremovesdebugmaps/threads,reduces31samples
3samples;preservesconfig/MAC/network/nativechecks. Notprepared/usedyet. Next
preparewithnewbackend/current263helpers,clock2,cap1000/filter1/pool-31/-30/900,
4800nestedtwoCPU,basesnapshot004,consoleclean263 andnativebrowserpayload024.
Useplainnamespacetemplate(discoverexisting)forprepare-board-local/prepare-board.
ThenrunlocalandSSHselectedsamepreparedidentity,actualfailurediagnostic257now
included. Don'tmutateanyfrozenoldinput. Independentagentsstillquota-blocked.

15:28 UTC22 October: cycle263 COMPLETE PASS onproductionbackend263, fourboots
provision245462ms/run0308182/run1371920/run2247353, eightCoreaccept/restart
checks andallnativebrowser/configuration/update/diskchecks. This includesnew
interactive(eager)navigationpolicy andprecommitrollbackscopefix. Backend263
binarya139b1c6afc70e993bd88d009e84d05af3a6d65c96dcd014ef6f6ea23f1a3db1/9791312.
CompiledASIC263 PASS859responses. Authorreviewselected-browser-policy-263.md.
Preparedcleanclock2-263processhash40339727245a633cbbcfd30f7d42c88bb3aa59bfeb2b0e97748e3b7c6e1c762f.
Consoleclean263SHA c166b30558dc562248dfd4711b51a6d2d452e56cbcd6a1c1688c73c56c337676.
Localclean263 ACTIVE28562 selectedlocal-412c311c390349568a3f244ab633bb39bc16bf71e456e7362a57b17ac45c4202.
SSHclean264 ACTIVE61325 selectedrun-8795c04fb0a282f9fb3244e8a06202bc4b416df00d74f8fdf858f6a600556c87.
Bothusefrozen263helpers/backend263/samepreparedidentity, plainnamespace
scenario template dashboard-driver-156, browserpayload024. Lastseen~1383s,
nativeHTTPnotyetfinished,noerror. Livebrowserdiagnostics257included. Use263
reporter atfinish. SSH253 COMPLETEFAILED timedout4800087ms/fulltrace;
matching252extractPASS; nativeassetHTTP200/curl28failure. Strictreport/retainer
stillTOEXECUTE; noCoreclaimfromtimedoutresult. Independentreviewquotaunchanged.

T17originalARMoracle nowworks. NewRustexamplet17_core_oracle +Cfixture
simulator/firmware/fixtures/bm1397-core-oracle.c. CneededARM32ABI/syscalls;
installedRusttargetslistnative/AArch64/macOS/windows,butnoARM32core. Embedded
assemblyincbincopiesoriginal64bytereadroutineat117936andwriteat118000.
Toolsetupfailuresretained265..270: genericELFclosurerejectsrelativeRUNPATH;
exactRust1.99LLD/LLVMarecapturedinprivateoriginlayout(nochangegenericchecks).
LLDneeds--threads=1under16processbound. Clang23defaultPIErequiredexplicit
-static/-fno-pie. ELFmustbeARM32ET_EXEC,nodynamic/interpreterheaders. QEMU11.1.2
fullARM/kuserpage4GiBreservationcannotfit2GiBAS; -R256Masserts, -R0stillneeds
fullrange. Dedicatedoracleprofile6GiBvirtualAS/60CPU/90wall/64MiBfile/32MiBtmp,
1MiBgueststack,noegress. Versionsandtoolhashesareinretainedinputs/logs.

Oracles271,272,273,274 PASS49152computedwords:32768reads+16384writes. Clang
-Wall -Wextra -Werror and-fsanitize=undefined -fsanitize-trap=undefined; both
compileandexecutionstderr empty. Originalinstructionsnotinstrumented.272+
checksoneexactcopyeachroutineinELF; read57a96a1cf41038322a9ce46cf615bf161fdc528f5089a4c0e50bfeff4266b715,
write6914a2d19ac20c011eb801a32b04fc859604490ed2b83451968038414a8ecc89.
274addsderivedELF/outputhashes/inputorder. ELF250d4fe9a95220e5f3b23b4e36478bcf144075b1613df42ea10a7bd786886fa8/1456;
words7527abdd4015bbafa0dc1121733a9ff20c37799e142e11b6d2c69f57b45745d1/196608.
Newderivedfixturet17-core-packing-results.bin/jsonretainsfullvendoroutputs
andprovenance. Authorreviewt17-core-oracle-272.md,READMEcommands273/264verified.
Currenthelperoutputtarusesexplicitfilewhitelist. Nooriginalarchivechanged.
Theseareprotocoloracles,notboard/miner/miningpasses. Full274568tests/101bins,
strictClippy274PASS. Current275addsBM1397core_controlcodec;17scopedBM1397tests
PASS inclall49152vendorwordsandreservedmode/fieldbounds. Decoderseparates
encodedselectorfromphysicalcoregeometry. IdentificationChainstillrejects3c;
noacknowledgment/readinessinvented. Physicalcoreclockeffects/defaults/count/
readreplylayoutstillneedimplementation/evidence. EspeciallyHASH_CLKENbit3
isnotassumedactive-highdisable: ESPinit74leavesit0yetactualfirmwaremines.
Nexttracevendorcore0settingsandreadbacktoresolveeffects,thenimplementmodel;
useoracleoutputsforregression. Full275checksactive76920/clippy94731/fmtlinks20835.
Needreleaseall+sourcecheckpoint/freezeafterfinalcurrentchanges. Noexacttarget
completiongateclosed;matrixv11still79599rows/zerofullexactpasses.

Priority update 22 October from user side conversation: prioritize FPGA fidelity now. Exact selected bitstream/board validation is a dependency for the first exact-target pass. Resolve source/build provenance, 0.2.0 versus 1.0.2 behavior, inferred FIFO behavior, and missing Zynq fabric clock/reset connections. If matching RTL is unavailable, pursue an evidenced reconstruction and record precise missing inputs. Adapted S9 results remain regressions. Full matrix and gates unchanged. Existing local263/SSH264 tests continue. Agents still quota-blocked until25 October08:48; no repeated retry. Full275 tests/clippy/fmt/links completed exit0.

FPGA priority implementation276..279: new s9-fabric.patch adds4SLCR clock outputs and4active-low raw reset outputs; divider/source/staticgates fromexact archivedLinuxclkc.c SHAe0b7a555...ee2fc2. ResetsclearTHR_CNT/CTRL (upstream retained gate bug). backend276release SHA802a7a3cee523d7a2b746d9e8d8837b75a0cb791b2dff8a345d6545c952f7a5a/9837328. fabric-signals276PASS242transactions111clockstates7resetstates. qemu_ttc_reset newfabricmode277PASS128transactions/twoactualQMPresets; SAMEcheckbackend263FAILretainedTHR_CNT=1. Current279adds legacyS9IOFCLK1 clockinput andQ32scheduler, phase restartonclockchange, UART/FIFOstatepreservedwhilegated. Reference2020/miningpeer remainsfixedclock; independentASICclock scheduling stillrequired. fabric-uart newcompiledtest tests3chains/mid-byte gate/resumehalf-frequency/FCLK0independence/exactrounding. Build278FAILfabricboardpatchanchor; corrected279active65954. Snapshot279 containsallchanges. Clippy278PASS. NeedrunUART/legacy/ASIC/TTC/reset andUBSan, thenfullRustchecks/docs. Noexacttargetgateclosed. Selectedlocal263/SSH264stillACTIVE28562/61325butbothnativeHTTPreadinessfailat~3600s. GuestASICtracetimeonly~62safter~1hourwall; actualpoolsubmissions present. Newcleanconsole didnotreachHTTP; do notclaimdashboardpass orrepeatsameconfiguration. Preservefinishthenreview topology avoidingnestedTCG ifpossible. Existing inheritedAF_UNIX ethernet transport couldconnecthostboard toisolatedworkerVM withoutnestedCPU, but actualworkerstartup/artifact ownership muststillbetested. OldSSH253strictreport276FAILasexpected.

Latest user instruction22October: STOP at each completed milestone for user review and approval before next. This supersedes uninterruptedfulltaskwork. Overallfullspec unchanged. Firstmilestone usable adaptedS9 nowunderactivefreshverification. NewRustCLI `md-sim demo s9 --check` validates31frozenprimaryinputs PASS280, `--out NEW` invokesexactfrozen263s9_dashboard_cycle withoriginalfirmware/nativebrowser/signedupdate/fourboots/eightCorechecks. Poolbackendcopiedbyteidenticallyfrom/tmp to durablework/simulator/demos/s9-adapted-inputs-280/pool-backend. Manifest simulator/demos/s9-adapted.json. CLIbuildPASS280 andClippy280PASS. ACTIVE58828 demo work/simulator/demos/s9-review-280 (ETA~20min from263). Stages currentlyprovision. Dashboard onlytestedinside privateVM, nohostURL duehostAF_INETdeny; donotclaimremote263/264pass. Milestones durablework/simulator/milestones.md withacceptance/blocker/nextaction/estimates. Needfinishdemo, authorreviewCLI, verifynegatives, docsverifiedstart/accesslimits thencheckpointandSTOP foruserapproval.
FPGAbackend279 alsofailedmanualpatchanchor evenexacttextcontextmatched. Rootcauseasymmetricmanualdiffcontext; regeneratedcanonicaldifflib3context boardpatch andhostdryrunPASS. Newcheckpoint280, backend280release ACTIVE36810. UARTclockimplementationandtests needexecution/sanitizer beforeclaim. FullRust280 ACTIVE8790. Existinglocal26328562/SSH26461325stillrunningnear4200s, bothHTTPreadinessfailedat~3600s. Mustcollectoncompletionandmarkfailed, noCoreclaimsiftimedout. Remainingfrozenbinary/sourcecheckpointforfinalmilestoneafterdocs/testchanges.

22 October16:31 UTC checkpoint283/284: demo280 FAILED at restore boot browser. Original WebSocket opened and connection_ack arrived; no next samples within20s deadline. Browser HTTPpreflight64806ms; build/test workload coincided with supportVM startup. Host contention is a hypothesis only. Prior4Coreaccept/restart checks passed. Full trace/exited1, no milestone pass. Unchanged frozen CLI/helper/backend/manifest repeat284 now ACTIVE11858 at work/simulator/demos/s9-review-284. Complete all checks before browser phase; do not extend deadline or alter assertions. Latest user still says STOP after completed milestone for review. README quickstart added but final passing report still required.
Local263/SSH264 now finished: inner processes exited0,4734072/4733991ms, full traces. Selected native HTTP failedstatus000/curl28, browser nottested, TLSnotpublished. Matching263 extractionPASS/strictouterreportFAIL. Localretention283PASS. SSHretention283 rejected exported result hash. Author found export reserialized validJSON. Fixed preserve verified rawbytes +regressionPASS. Re-extracted in separate board-ssh-clean-264-export-283; retention againfailed because sharedreporter also rewrote result. Fixed reporter to preserve semanticallymatching originalbytes and reject changed/invalidsaved records; meaningfulregression added. Old failures remainunchanged. Full283tests571/101PASS,fmt/clippy/releaseall/linksPASS. Full284ACTIVE1372 after reporter fix. Needrebuild/retestcorrectedretention,finaldocs/checkpoint/freeze. Releaseall unifiesextraCargo features and changesCLIbinary; `cargo build -p md-sim --release --offline --locked` restores exactlytestedCLI0f36a5779b450ca838ed8b2c5f23835fc9bb2e02db15c6229f75fbdf2c624bd6/3833080. Verifybeforehandoff.
Backend280release8fe3cd60c79213e695b5998c7084de64d77d91e6ce67ba0184bd367e42727b1b/9837328,UBSan0bc112bfa1b7fa8304f2286b598ddf079d65af4dacb301ece02a903e0a40cd8e/20008472. Fabriclegacy/ASIC859/fabricsignals2421117PASS; fabricUART28245/QMPcleanexitPASS,backend276baselineFAIL. Fabricreset282128/tworesetsPASS. UBSanUART/reset/SWDT60/TTC385allPASS. Authorreview work/simulator/reviews/adapted-s9-milestone-283.md, exactFPGAstillopen. Independentagentsstillallquotaerroredconfirmed. No exacttargetpasses.

16:40 UTC: demo284 still ACTIVE11858, provision exited0/245604ms; signed-install run0 exited0/308030ms/fulltrace, postboot audits/Core in progress. Full284 checks575tests/101binaries PASS, fmt/clippy/releaseall/guide-linksPASS; final `cargo build -p md-sim --release --offline --locked` reproduces originalCLI0f36a5.../3833080. Exact testedCLIcopied to demos/s9-review-284/cli-binary. No more builds/tests during upcomingbrowserphase. FinalREADME/evidence/checkpoint stillpending; do notstopuntilmilestonepass.
CorrectedSSH264retention284 PASS in board-ssh-clean-264-export-283/retained-284; originalresultbytes/hashpreserved. Bothlocal263/SSH264 session0 freshaudits PASS2jobs/cleantransition/difficultychange/actualhardwarecomputedserialresults; local14772hashes/4matched/89verified,SSH14527/5/91. FourseparateCoretests PASSaccept/restartheight0→1→1 at parent/core-session0-candidate{0,1}-284. Two runs sharepreparedidentity and secondcandidatehash, do notcallfourdistinctblocks. Localfirst28081508... second3beb3bf4...,SSHfirst0464cd8f... second3beb3bf4.... Otherpool sessions notconformancepassed;selecteddashboardsstillFAILHTTP000/curl28,strictouterFAIL. Reviewupdatedwithbytepreservationfix andtests; independentquotaunchanged. Disk16GiBfree,avoidlargeunnecessarycopies.

16:50 UTC: unchanged demo284 native browser PASS inside run1 guest record (MD_SIM_BOSER_BROWSER_RESULT passedtrue); dashboardrestoreAPI alsoPASS. No other builds/tests ran during browserVM startup orbrowserphase. This supports resourcecontentionhypothesisfor280 butdoesnotproveit; retainfailure280. Needfinishrun1storage/mining/Core,run2restoredsettingsreadback/mining/Core,thenouterassessment. Actualpriorcycle263total33.66min,20minestimatewasphaseonly;README/milestonescorrectedtoabout34min. Finalmilestonereport/currenttests575/sourcefreeze/checkpoint thenSTOPforuserreview. Sourcecodeunchangedafter284checks;READMEfinalpasslinksremainpending. Activeprimarysession11858only;allotherthisturnchecksfinished.

17:09 UTC milestone handoff checkpoint285: demo284 COMPLETE PASS, outerCLIexit0/full_specification_complete=false; completecycle33.72min. Four originalfirmware processes exited0/fulltraces: provision245604ms,write308030,restore370555,verify247579. EachauditPASS2freshjobs/cleantransition/difficultychange/computedCRCcheckedhardwarework. Eightdifferentblocks accepted by8separateCore nodes, height0→1→1afterrestart. Nativebrowser/actualpagetables/WebSocket2samples/sameoriginisolation/signedupdate/configwrite-readback-restore-and-restart/SDchecksPASS. Exactphysicalprofileunresolved;reference3×63BM1387,1000H/s,explicitfilter4/adaptations. Neverclaimfulltaskcomplete.
README now testedCLIquickstart--out demos/s9-review-284 (newpathrequiredonrepeat). Report work/simulator/reports/adapted-s9-milestone-284.md; inputs demos/s9-review-284/input-manifest.json,run/driver-input.json; retainedCLI0f36a5779b450ca838ed8b2c5f23835fc9bb2e02db15c6229f75fbdf2c624bd6/3833080matchescurrentbuild. Frozenhelpers/backend263unchanged;newFPGAbackend280separatecomponentregressionsonly. Sourcecheckpoint285412files/4310903bytes;currentfilescheckedbyteidentical. Sourcefreeze2853644source/vendorfiles,identity709ee651f273771da75e98ce2315599cd3340fbde3e07390af4893f66285937e;verifyPASS. Full284575Rusttests/101bins,strictclippy/releaseall/formatPASS;finalfmt285/guide-links285PASS;newreportlinks28630PASS. milestone-checkpoint-285.jsonpins35report/input/checkartifacts. Independentreviewquota stillblocked;authorreviewseparate. Allprocesssessionsstartedthiscontinuationfinished. STOP NOW foruserrequestedmilestonereview;doNOTstartmilestone2withoutapproval. Fullmatrix79599rows/0fullexacttargetpasses;exactFPGAandcomponentmodels,selectedremoteTLS/SSHdashboard,cleanfirmwarepackaging/fullmatrixstillopen. Prior280browserfailureand263/264remotedashboardfailuresremainretained. Do notweakenorrelabelfailures.

22 October: user approved milestone2 with “nextt step>”, then suggested old Braiins FPGA simulations. Milestone1 historical milestone file retained at reports/milestones-at-checkpoint-285.md with unchanged SHA355110ca...ac829. Inspecting archived Braiins s9io0.2 HDL and fve/s9io_tb.sv (Vivado AXI VIP dependency). Existing Rust tests already reuse literal vectors. Need execute HDL independently and compare unknown FIFO/reset edges; selected2020 bitstream still unmatched. All three review agents remain quota-blocked; no new retry.

23 October recovery288: previous tool calls failed with EDQUOT during sandbox setup and apply_patch failed writes/deletes. Now shell works: root100GiB and tmp11GiB available, inodes available; create/write/delete probe passes. Cause remains unresolved. Interrupted s9_bitstream_probe.rs and s9_boot_bundle.rs are byte-identical to core-checkpoint285; no truncation. New bitstream/series7.rs and module declaration exist but are not yet tested. Resume their tests/integration; preserve frozen demo284. Recovery evidence: reports/quota-recovery-288.json.

Recovery288 complete. New strict Rust Series7 packet inspector runs inside existing s9_bitstream_probe with --series7-idcode03722093 (separate arguments). Selected2020 image980532de... passes both stored CRCs16726811/e3ad7ea5; older image569597aa... passes65f36559/e3ad7ea5. Both546packets/520352FDRIwords; distinct FDRI hashes2b3cb98f.../0b1e1429.... Corrupted one-bit FDRI with valid recomputed gzip fails configuration CRC as required. Unit5tests include3232single-bit data mutations and all truncations. S9prepare now requires packet checks; fresh sd-bundle-288 passes and all7boot components match sd-bundle006 byte-for-byte. No new firmware execution/mining/board pass claimed. FullRust580tests/102bins, strictClippy,fmt,affectedreleasebuilds and511guide-links PASS. Evidence reports/series7-inspection-288.json; author review only, prior agent quota still unresolved. Recovered ProjectXRay part.json via provider text has exact published SHA1754ec1c...d095; inputs/provenance under qemu-s9/series7-inputs-288. Next expand5144frames plus8paddingframes using pinned part data; then matching logic/reset/netlist or HDL oracle. Frame decode is not implemented yet. Fullmatrix79599rows/zeroexactpasses unchanged.

23 October user correction289: Do not stop at a status answer or a component test. Continue milestone2. Clean expendable files during work. Removed only inactive Cargo debug caches clean-target210/debug and check-target085/debug; release helpers, archives, reports, source and captured inputs remain. Exact record reports/build-cache-cleanup-289.json. Independent agents still show unchanged quota until25October08:48; no repeated spawn. Next implement strict FPGA frame mapping, continue real path and cleanup.

23 October289..291 continued after user correction. Removed12.2GiB inactive Cargo debug caches, then247 verified duplicate tmp QEMU-build inputs (2797918529bytes). Journals preserve exact restoration copies; all31 demo inputs stillPASS. Cargo dev nowdebug0/incrementalfalse; fullcheck output~550MiB instead of9.8GiB. No archive/frozen release/evidence removed.
New Rust series7/frames.rs pins ProjectXRaypart1754ec1c...; strictschema/unique keys/bounds and FAR order, accounts5144frames+8zero padding frames, checks every13bit frameECC. Actual selected+older images bothPASS isolated probes291; freshbundle291 includes fpga-frames.json and unchanged7firmwarecomponents. Derived actualimage negative flips firststoredECCbit and recomputes validpacket/gzipCRCs; strictmapping correctlyFAILS expected0001computed0000; retained. Full586tests/103binaries,fmt/clippy/releaseprobes/514linksPASS. Report reports/series7-frame-mapping-291.json pins sources/executables/reports. Additional Series7 fix rejects zero packet words beforeDESYNC. Source tologicstillOPEN. Complete tilegrid SHA db16874f...13479ee unavailable via primaryraw/staticproviders; providerfailures retained series7-inputs291/reference-provider.txt. GHDL/Vivado missing; agentsstillquotauntil25Oct08:48. Do not retryunchangeddownloads/quota.
Next concrete implementation: separate ASIC scheduler fromfabricedgeclock. CurrentC md-sim-s9io.c rejectsclocksource+BM1387; Rust md_s9_advance tiespeerphysicalns toregisterscore_period5ns. Needcommon fractionalQ32time advancement withindependentpeer deadlines, gatehaltscorebutASICUART/hashcontinues, same-timestampold-levelsampling, timestampedpower/reset andfixedclockregression. Tests crates/md-sim-qemu-core/tests/s9_chain.rs andCqtest+UBSanrequired. Noeditstothatpathyet. Remote263/264stillfailed; do not repeatnestedTCGunchanged. Milestone2NOTCOMPLETE; continue, nohandoffatcomponentcheck.

Active294: implemented physical ASIC scheduler in crates/md-sim-qemu-core/src/timing.rs; commonold/newadvance Q32clock+explicitASICdeadline/advance/pin APIs. Wire now_ns getter, Cbinding newclockspan/independentpeertimer, onecapturedtimeforClockPreUpdate+ClockUpdate. Newexplicitmachine fabric-clock-reference=on offbydefault connectsFCLK1 toBM1387reference. PureRust10s9chain testsPASS293 including fractionalclock7.5ns, baud-preserving5/10ns, gatedserial441byteswithoutFPGAqueue, freshhashingwhilegated. Tightenedthatlasttest294requires16additionalhashesaftergatebegins; fullchecksactive. ExistingfixedclocktestsPASS.
CbackendrebuildUBSan294ACTIVE session3798; checkpoint294419files pinsnewlibrary. Command in checks/asic-clock294-qemu-build.log, backend-build-294-ubsan. QEMUstaticpatchdryrun293failedbecauselegacyCPUtimerpatchrequiresfuzzalreadyusedbyoriginalbuilder; retained. Repeat294 usesexactoriginalpatchoptions andnewBM1387/Fabricfuzz0; passeswithBMparent923433... andderived89abaa2623a9a5a93311baa1c2f6c35c204a1bb763db5d7de6ced7ff4a938cf6; updatedbothhardcodedderivedhashes. Patchedfinalboard943b7cf7ac9f2b93b246b0ca4e073c4f442ee43ab4cb10172027056be03bcd21. FullRust294 runningseparately. New qtestmode bm1387-fabric checks3×63ASICs before/afterFCLK1half+guestUARTdivisoradjust; needscompiledexecution. Also runexistingbm1387,legacy,fabricUART/reset/SWDT/TTCregressionswithUBSanandbuildreleaseafterfixes. No newfirmwareboots yet andno exactmilestoneclosed. Latest resource110GiBfreeworkspace/14GiBtmp; debugcache603MiB. Continue, do nothandoffhere.

294compiledUBSanPASS: backendSHA18aa8274e4c35e639d137279e6ad7b19fb872b161ac5f03fdfa1459412e7844c/20016824. Newbm1387-fabric andfixedbm1387 qtestsPASS; theirstdinEOFtimeoutterminationisexpectedcomponentpolicyandnotbootclaim. FabricUART45/QMPcleanexitandfabricreset128/tworesetPASS. SWDT+TTCregressionsACTIVE51468; oldbackend280negativefeaturetestACTIVE21825. Full294590Rusttests/103bins,fmt/clippy/515linksPASS.
FRESHfirmwareACTIVE31465 in qemu-s9/native-asic-clock-294: s9_boot_bundle boot sd-bundle291 spl sd2022-v1 backend294UBSan budget360 pool300, cap10filter1 difficulty-31,-30, regtest-genesis, batchconsole133, monitor031, savedSDautomated-update-cycle004/provision. Durablepoolbackend demos/s9-adapted-inputs280/pool-backend. PrepPASS1156ms. Needwaitfinishthenrun s9_asic_audit rates + regtest and node_prerequisite_probe Corebinary NEWOUT AUDIT CANDIDATE(0,1); require nodeheight0→1→1. Frozenhelpers263provideoracles. Do notclaimnewboot/mininguntilactualresults.
Authorreviewfoundx86-poolbuilder skipsSWDTbutunconditionallyappliesdependentfabricpatch, causingSLCRparenthashfailure. Fixedonlyfabricguardtosuccess&&!pool_x86, preservesARMpath; clock_contractreportnew andfabricfixturedescriptionupdated. ThislaterhelperchangeNOTincheckpoint294andneedsbuild/fullchecks; source/library/Cmodelunchanged. Beforeheavynewbuildfinishcurrentfirmwaretoavoidcontention. NeedreleasebuildandfullC/firmwaretestevidenceafterlastfix,currentdocsFABRICstillmarkscompiledinprogressuntilfinalreport. Milestone2OPEN;continue.

23 October 295: firmware294 completed cleanly in307147ms with complete traces. Independent rate audit checks1323 computed hashes,441perchain, all unique monotonic grants below10H/s cap. Fresh audit verifies2jobs, clean transition,difficultychange,8matchedcomputedserialresults. Both Core295 candidates ACCEPTED and retained after restart,heights0→1→1; hashes3a789e0f01a0465973dcdfd5ea4bde282bd925a4091e17e9258176bae12b0220 and1134dee70028b7abee3f86fc96651ab2160385d7468c05c7042ce4c6b186c390. Sessions12987/14546/71789closed exit0. Report reports/asic-clock-295.json pins backend/checkpoint/model/checks/audits/nodeoutputs. This is adapted fixed-clock reference firmware; optionalFCLK1referencewiring only compiledqtest. Milestone2 stillOPEN. Workspace110GiBfree,/tmp14GiB,debug603MiB. Agentquota unchanged, no repeats. Next rebuild corrected helper,freshcheckpoint and actual x86 support+releaseARM builds; continue FPGA/exacttargetdependencies.

296 backend follow-up PASS: ARM release dfaf191122b2d19f59412795bc3f0cac1c1d024b21046b0d16d815e7521c3d07/9841424; x86 support UBSan a7acdaf81c675acea75969a96e41ec40424aa5f0235569c31f3585c12e478bb9/19064864. Actual x86 build confirms corrected ARM-only SWDT/fabric guard. Isolated pool-link296 PASS33connections/32invalid submission rejections/reconnects; this is transport-only, no mining claim. Release fabric-uart296PASS45 and ASICfabric296PASS1257. Checkpoint296 retains exact source. Cleanup helper initially rejected repository build paths (cleanup298.log, no deletion). Added strict numbered repository backend/pool build patterns, negative path testsPASS; helper299 reclaimed153 verified duplicates/2095049160bytes with restore journal reports/duplicate-build-input-cleanup-299.json. Originals/evidence/release binaries retained.

New concrete FPGA gap297: pinned upstream hw/dma/xlnx-zynq-devcfg.c SHA59592eae... drops destinationffffffff transfers without reading memory and setsPCFG_DONE after interrupt writes. Old adapted runs cannot prove actual FPGA configuration validation. Source extraction/provenance retained qemu-s9/pcap-source-297. Implemented optional strictPCAP path: Rust Port validates pinned container+part/CRC/ECC, compares actual little-endian DMA words acrosschunks, programmingreset/enable/failure latch, fullimage or exactsyncsuffix only. Cbinding readsactualguestRAM, onlysetsdoneafterallcheckedwords, status/maskwrites cannotinventcompletion, unsupportedpathsstopwithmodelerror. No FPGAl ogic reconstruction or physicalPCAPtimingclaim; upstream synchronousDMAretained. New configuration-contract bootoption capturesselectedsystem.bit+part and manifest. New qemu_ttc_reset pcap mode exercises full/split/sync DMA,twosystemresets,programmingreset,corruptedguestmemory rejection.
Tests297:4port+6framePASS. Clippy297foundlazyinsert;fixedandClippy298PASS. NewUBSanbackend298 ACTIVE32971, checkpoint298pins428?files (inspectactualmanifest). FullRust298 ACTIVE93833. Needfinishbuild,runqemu_ttc_reset BACKEND NEWOUT pcap (107expectedtransactions), correctfailures, then actualfirmwareboot withconfiguration-contract at10H/s,freshwork andtwoCoreaccept/restartchecks. Need verify all three streamhashes, not onlyfirst. CompiledC andfirmware tests NOT YETPASS forPCAP; do notclaim. Latesthelpercleanupchangeaftercheckpoint298needsfinalchecks/checkpoint. Keepdemo284frozen. Milestone2stillincomplete;continue.

PCAP301 compiledPASS107transactions/twoQMPresets/threefullconfiguredstreams, allthreeactualhasheschecked; corruptedguestRAM atpayloadbyte20 stopswithmodelerrorandsetsnodoneflags. Backend298UBSan SHAfb498782cb37345578dc601053091e6de9734a701669f232d33f3d10b1bd6d2c/20189560. Priorpcap300FAILcaughtignoredQEMUglobalshortsyntaxbecausexlnx.ps7-dev-cfgcontainsdot; replaced withdriver=...,property=...,value=...; failureandoldbackend299negative retained. Newpcap_audit checksreset/enable/orderedDMAcount/digest/completion, rejects missing/false/extra/corrupt evidence; bootconfiguration-contract requiresitaftercompleteexit0. Test302PASS,Clippy/fmt302PASS,fullRust302PASS(deriveactualcountfromlog). Livefirmware native-pcap302 ACTIVE29682 cap10/filter1/twofreshjobs; actualU-Boot PCAP2083740bytes matched49d0faa4... payload andguestenteredLinux. Needfinish,mining/ratesaudit,twoCoreaccept/restartchecks. Do notclaimthisrunpassedbeforethat.
Cleanup299+302 reclaimed204duplicates/2793398880bytes (~2.6GiB) fromfinished294/296/298buildinputstores, restorecopies pinnedinreports. Currentworkspace109GiBfree,/tmp14GiB,debug615MiB. Nooriginals/frozenhelpers/evidenceremoved. NewprimaryPG1645.0reference obtained asproviderPDFtext only; reset-reference-302.json records16clockreleasegaps andunresolvedgeneratedparameters. It explicitlywarnsIPIntegratorautocomputesvalues. Do notguesslegacyorselected2020resetdefaults; generatedXCI/wrapperorindependentHDL/measurement stillrequired.


23 October checkpoint305: PCAP worker304 finished successfully. It used the identical frozen process from native-pcap302, through a separate local IPC worker. This is not SSH or an external host. Destination artifact hashes and exact retrieved worker-result hash match cleanup acknowledgment; run cleanup succeeded. Shared configuration audit passed full2083740-byte transfer. Independent frozen263 audits passed1652computed hashes, 2jobs, clean transition, difficulty change, 9matched/9verified results, 10H/s cap. Core worker304-candidate{0,1}-305 both accepted and retained after restart (height0→1→1). Original302 Core303 checks also passed. All historical reports retained.
Release backend304 built successfully: a2ccec085c89eb297adc5c0bbf5dc672249a5dcff6a552c11fdc614e30720af0/10017552. Compiled pcap-release305 passed107transactions,2QMPresets,3image hashes and changed-memory rejection. Same C/Rust device model as UBSan298; checkpoint304 pins inputs. Cleanup305 reclaimed51verified duplicate inputs/698349720bytes with restore journal. Canonical077 retained.
Author review305 hardened the shared auditor: require initial reset; reject 32-bit DMA extent overflow and competing/noncanonical configuration properties. New tests reject retained-reference tampering and symlinks, and prove retention after store cleanup. Full305: 606tests/104binaries PASS; fmt, strictClippy, release firmware examples PASS. Original firmware/helper302 inputs unchanged. Fresh same-process replay pcap-local306 is now running to exercise hardened shared auditor. Need collect it, independently audit rates/regtest and two Core candidates; then complete PCAP aggregate report, author review and source checkpoint. Source models need no rebuild for auditor-only fixes. Independent agents remain quota blocked. Full matrix79599rows/zero full exact passes, milestone2 incomplete.

23 October checkpoint307: Added explicit configuration-contract to s9_dashboard_cycle. It passes this option to provisioning and all three test boots. Each phase must retain a passing configuration audit tied to that exact result, backend log and selected image. Driver schema5 records the policy. Negative tests reject missing, failed or substituted evidence. Frozen nine current Rust helper executables at work/simulator/pcap-target-307/release/examples; manifest.json records19755600bytes and hashes. Do not modify those frozen binaries. The full signed-update/native-browser cycle is ACTIVE session85075 at qemu-s9/pcap-dashboard-cycle-307 using release backend304 and bundle291. This remains the adapted reference; 1000H/s cap/filter4 as prior dashboard cycle, explicit strictPCAP on all boots. Do not claim its pending phases passed. Avoid concurrent heavy builds during browser startup and phase.
Hardened local replay306 remains ACTIVE session28047, same302 process/cap10H/s, new suite reporter305. Final fmt/clippy/fullRust307 ACTIVE15011. Prior checks305 passed606tests/104binaries. Source model unchanged from298; release304compiledPCAP305passed. README/docs and reports/checked-configuration-305.json now retain both completed local302/worker304 plus all four Core acceptance/restart records, transfer and cleanup hashes. Author review reviews/checked-configuration-305.md is not independent review. Guide links308 passed. Next collect replay306, run independent frozen263 audits and two Core tests, complete307cycle, investigate any failure without weakening assertions, then final source checkpoint and updated report. Full milestone2 and matrix remain incomplete.

Checkpoint309/310: Hardened local replay306 PASS;1316computed hashes/2jobs/clean/difficulty/8matched8verified, bothCorelocal306-candidate{0,1}-308accepted andretained. Full308607tests/104binaries,strictClippyPASS. Clippy307 caught8argument helper; grouped epoch/configuration into BootPolicy and reran. Frozen cycle307 driver source saved at pcap-target307/source/s9_dashboard_cycle.rs, hash matches running driver-input. Its frozen binaries are unchanged. Cycle307 passed provisioning +2Core tests and is in signed-install phase; no final cycle claim.
New concrete IRQ defect: expanded pcap_qtest checks actual GIC input8, mask/unmask, independent DMA W1C, programming reset and system reset while asserted. Test309 fails oldrelease304 exactly:status0butIRQstillhigh afterprogrammingreset. Failure retained qemu-s9/pcap-irq-old-309. Fixed strictCbinding to refreshIRQafterprogrammingreset and aftersystem-resetregisterreset. Newderivedsources/provenance qemu-s9/pcap-derived-309; original297derived inputs preserved. Newtestexpects117transactions/8IRQedges/2QMPresets/3configuredimagehashes. Needverifyactual117count, investigate ratherthanweaken. Sourcecheckpoint310pins428files/currentlib. Build310firstCLIinvocationFAILwrong sanitizer syntaxbeforeoutputcreation; corrected realubsan build nowrunning. Need finishbuild,runnewIRQtest,fixissues,then actualfirmware freshmining/Core onfixedbackend and release/nativecycle asrequired. Currentcycle307usesprevious304backend and cannotstandinforfixed310identity. Do notupgrade itsclaim. Milestone2/fullmatrixstillincomplete.

Checkpoint312: UBSan backend310 built cleanly SHAe5f0113a774b7af441d5361ddd7ae53db8c67bd8d01687c85d84f63f4ebb822c/20189560. Expanded test311failed both310and304 at coldINT_STS:actual0x88023000 vs declaredreset0x88020000. Investigation: register_reset invokes DMA_DST_LEN.post_write; upstream enqueues zero-lengthDMAand falselysets DMA_DONE/DMA_P_DONE. Addedstrict-only device_is_in_reset guard in newderived312 sources; no flagsinvented/expectationsweakened. Correct118transactiontest explicitlychecks cold/warmresetstatus0x88020000,8GICedges,2resets,threeimages; earlier309stillpinpointsprogramming-resetIRQfailure. Sourcecheckpoint312containsguard/currenttests. New312backend NOT BUILT yet: deferheavybuilduntilbrowserphase307finishes. Cycle307 currentlyrun1/browserevaluation (nofinalresult yet at03:27UTC); active85075. All310build/tests closed. Full310607tests/104binariesPASS; finaltest311addsonecoldread and warmresetliteral fix afterthat, so needfinalchecksagain. Next afterbrowserdone build312UBSan, run118test witholdnegative311retained, resolveanyremainingfailures, actualfirmwarefreshmining/Core underfixedbackend, andcleanfinishedduplicatebuildinputs310/312withjournal.

03:29UTC: Cycle307 run1 completed, original native browser observation passedtrue, originhttp://10.0.2.15, profileisolationtrue; strictconfigurationaudit alsoPASS. Proceeding restore disk/mining/Core thenrun2; finalcyclepending. Heavybuildsresumed afterbrowserphase: correctedbackend312UBSan ACTIVE36066. Fullfmt/clippy/tests/releaseexamples312 ACTIVE (collectsession fromtool). Afterbuildtest qemu_ttc_reset BACKEND312 NEWOUT pcap;118tx/8IRQ/2resets expected. Ifpassed runactualfirmware native-pcap313 at10H/s/filter1 withnewrandompoolinput, independentaudits/Core. CurrentREADME/newreportsremainexplicitaboutpendingIRQfix andall exactgates.

Cycle307 COMPLETE PASS:fourboots245412/308519/365432/246311ms,allstrictconfigurationauditsPASS,nativebrowser/login/assets/livedata/isolation/signedupdate/configrestore/persistencePASS,eightDISTINCTCoreacceptedblocks retainedafterrestart. Aggregate reports/checked-configuration-dashboard-307.json hashesallphases/inputs andfrozen307driver source. Itusesold304backendanddoesNOTreplacefixed312/315identitytests. Cycle85075closedexit0. Freshnative314on312closedexit0,configurationPASS;independentrate/regtestaudits ACTIVE56076,then2Corechecksneeded. Source-ref315negative on312provedCFG.DISABLE_SRC_INC ignored:validimagewasacceptedwith0x3004 despiteforbiddenincrement. Addedstrictguardthatstopswithmodelerror(noinventedhardwareerrorflag); actualkeyholeDMAstillimplementationwork. Newmodepcap-fixed-source shares118tx/8IRQ suite andchecks explicitunimplementedrejection. Source315/provenance andcheckpoint315 retained. CorrectedUBSanbuild315 ACTIVE79224. Needrunbothpcapmodesafterbuild,freshfirmwareandCoreon315,releasebuildandupdatednativecycle. Cleanup314removed102duplicates/1396699440bytesfromfinished310/312builds. Canonical077intact.

Checkpoint316/317: LatestUBSan315 SHA530793d453142bcf0c93c9235a12de8c33a437bbe91b3264b6ecee777ef5f30a/20189560. Bothpcap-irq316andpcap-source316 PASS118transactions,8GICedges,2actualresets,3images. Fixed-sourceguardistestedrejectionofunimplementedcontrol,nota hardwarekeyholepass. Full316fmt/Clippy/607tests104binaries/releaseexamplesPASS. Native314on312independenttestsPASS1661hashes,2jobs,clean/difficulty,14matched14verified;Corepcap314candidate0/1-316acceptedretained. Aggregate reports/configuration-reset-review-316.json. Latestnative316on315 ACTIVE27733 atcap10/filter1; needsindependentauditsandCore. Release316backendbuild ACTIVE28563 usingcheckpoint315. Afterreleasepassesrunbothcompiledpcapmodes,thenfullnative dashboard/signed-updatecyclewithlatestmodel. Frozen11helperspcap-target317/release/examples(24031448bytes), manifest317; usefornewcycle317,do notmodify. Allotheroldtest/Core/worker/cycle sessionsfinished. Nextcleanfinished315/316duplicateinputsafterbuild; preservecanonical077 andallfrozenhelpers.

23 October, checkpoint 318/319. Native316 finished successfully with complete
traces. Frozen263 independent audits passed 1,310 computed hashes at the 10 H/s
cap, two fresh jobs, a clean transition and a difficulty change. Both Core318
tests accepted their block and retained it after restart. Hashes:
79a53a51010a8ce3b383969e45f7a4401706397027d6117f979373550924d7c8 and
1e5b220703ee5d1c9d89b1c58c4c8deca787bc3684e84442311405325c476a58.
Release316 passed both 118-transaction checks. Full native cycle317 is ACTIVE
session5188 with frozen317 helpers and release316. Provisioning and run0 have
passed execution/configuration checks. Do not claim the full cycle yet.
Prepared worker318 finished exit0 with full traces. Independent frozen263
audits319 are ACTIVE39110 and25789. Then run two Core acceptance/restart tests.
This is a local IPC worker, not SSH or an external host.

Cleanup318 passed its scoped tests and strict lint. It now recognizes completed
PCAP cycles and their provisioning Core checks. Three completed journals
duplicate-build-input-cleanup-318.json, duplicate-cycle-input-cleanup-318.json
and duplicate-core-input-cleanup-318.json record 4,301,770,508 reclaimed bytes.
Canonical077, original evidence, frozen binaries and active cycle317 remain.
The cycle307 Core source-evidence copies can be restored from the exact
original run files recorded in the journal. Do not remove those originals.

Next model work319 implements fixed 32-bit source reads, based on the primary
AMD CFG bit5 definition and the DevC 32-bit AXI interface. Unlike guard315 it
reads each actual memory word and preserves read side effects. It reports the
source increment mode. The auditor accepts historical increment-only traces
and marks those separately. Four scoped auditor tests and strict scoped lint
pass. The compiled test now requires146 transactions in fixed-source mode:
read eight repeated words at one address, switch to increments and complete
the exact image, then reject a full repeated stream at payload byte32. Normal
mode still requires118 transactions. Negative test on backend315 is ACTIVE
84654. Source checkpoint319 is complete. The new QEMU backend is NOT built
yet. Defer heavy compilation until cycle317's native browser phase finishes.
Then build UBSan319, execute both compiled modes, resolve failures, and run
fresh firmware/mining/Core checks before claiming this new model path.
Milestone2 and the full exact-target matrix remain incomplete.

04:28 UTC, checkpoint320. Both worker318 Core checks passed and survived
restart. Aggregate reports/configuration-firmware-worker-319.json verifies
the actual local316 and worker318 input/result/log hashes, both mining audits,
four Core reports, destination artifact identities and cleanup acknowledgment.
Worker318 checks1,318 hashes and9 matched/verified results; local316 checks
1,310 and8. These runs reuse the prepared seeds; no extra fresh-seed claim.
All worker/Core sessions are closed. Cycle317 browser passed; run1 is complete.
Cycle5188 is still active and must finish run2 and all checks.

Review found another concrete reference-policy defect. New control-recovery
test320 on backend315 failed: after rejected PCAP_PR=0, restoring valid control
bits without programming reset accepted a full image withstatus0x3004. Evidence
pcap-control-old-320 remains failed. Added Port::reject and a thin C interface.
Every rejected DMA now latches failure without discarding prior words. A new
unit test checks prefix retention, enable-only rejection and reset recovery.
The compiled control test requires136 transactions and a complete fourth image
only after reset. Fixed-source test requires146; normal test remains118.
An initial driver build320 failed OsString/&str conversion; fixed explicitly
through to_str. No assertion was weakened.

Derived C source/provenance320 supersedes319 with the latch call; both originals
are retained. Checkpoint320 captures all model changes. UBSan backend320 is
ACTIVE16197. Full fmt/Clippy/tests/release examples320 are ACTIVE85531; strict
Clippy has passed. Browser phase317 finished before heavy builds resumed.
After backend build: run pcap, pcap-fixed-source and pcap-control-recovery into
new output directories, investigate any mismatch, then fresh original firmware
at10H/s plus independent audits/two Core checks. Release build/full cycle only
after the model review fixes are stable. Keep frozen317 binaries unchanged.

04:39 UTC, checkpoint322. Backend320 UBSan built successfully with SHA
9ab7ad6604af0477c0e95e62bfd90de353b93732d808adaad744a03fce8bb7dd,
20,189,560bytes. Build diagnostics have no compiler warnings/errors. Compiled
normal321, fixed321 and control321 pass118/146/136 transactions respectively;
each has8 actual GIC edges and2QMP resets. Release321 built successfully and
the same three tests pass in pcap-release-{normal,fixed,control}-322. Full320
checks pass612 Rust tests in104 binaries, formatting, strict Clippy and all
release firmware examples. No code changed after those checks; README and
PCAP documentation were updated. Author review321 records exact limitations.

Cycle317 COMPLETE PASS. All four firmware boots passed, with signed install,
native browser, settings restore and restart persistence, strict configuration
checks, and8 distinct Core-accepted blocks retained after restart. Canonical
report is qemu-s9/pcap-dashboard-cycle-317/assessment.json and its driver-input.
It uses earlier release316. Do not reuse it for latest model320/321. The
attempted extra aggregate report317 failed on an incorrect browser JSON field
before report creation; browser evidence is in observations[0].passed, not a
top-level passed. Original cycle verdict remains unchanged. A source copy at
pcap-target-317/source/s9_dashboard_cycle.rs matches driver-input.driver_source.
No aggregate317 JSON exists. Use the canonical assessment instead.

Latest firmware321 on UBSan320 finished exit0/full traces. Its independent
frozen263 audits322 are ACTIVE32894 and59297. Next run two Core checks using
native-pcap-321/regtest-audit322.json. Frozen11 helpers at pcap-target-322 total
24,076,760bytes; manifest/source are retained. New full native cycle322 is
ACTIVE57701 on release321, using those frozen helpers and configuration-contract.
Do not change frozen binaries. Avoid heavy builds during its browser phase.
All compiled-test and backend-build sessions are closed. Cleanup321 reclaimed
another698,349,720bytes from finished UBSan320 inputs; canonical077 retained.
Next clean finished release321 inputs and completed cycle317 with the tested
cleanup helper. Never clean active cycle322. Milestone2 is still incomplete.

05:09 UTC, checkpoint329. Latest firmware321 independent audits322 PASS:
1,642 computed hashes, two fresh jobs, clean transition and difficulty change,
22 matched and verified serial results. Both Core323 candidates PASS acceptance
and restart retention; see bitcoin-core/pcap321-candidate{0,1}-323. Their hashes
are 6299c3e101e2d9b30b78ba27df0c829645dfe4be02b25dae29fcfca34926b570
and 3cb9831d0365f46962b6cd12734a8fc948979690a708189ed742e75cb8fef17c.
README, progress and author review321 now link these canonical results.
Cycle322 remains ACTIVE57701; run1 browser passed, final run2 is active.
No heavy build ran during its browser phase. Do not clean active inputs.

Completed cleanup: release321 build inputs 698,349,720 bytes (journal323),
finished cycle317 inputs 2,651,901,804 bytes (cycle journal323), and its Core
duplicates 250,773,936 bytes (Core journal325). Earlier sanitized build320
cleanup321 removed another 698,349,720 bytes. Protected canonical077 remains.
At 05:03 UTC the workspace had 89 GiB free and /tmp had 14 GiB free.

The separate native-worker topology is now implemented as worker_channel_probe.
It runs a real frozen263 IPC worker beside a private x86 client VM in one
strict ProcessSpec. A new bounded request_device API uses the existing worker
protocol on an explicitly checked character device. The VM verifies its
virtual port name/device identity, transfers a fresh 1 MiB object, checks the
worker destination hash, downloads all bytes, rechecks worker identity and
requires a shutdown acknowledgment. This is a transport component, not SSH,
firmware execution, native dashboard or an external physical host pass.

Failed probes remain visible:324 used the wrong confinement memory contract;
325 requested unsupported optional QEMU sandbox;326 socket chardev failed
host getsockopt restrictions;327 found the minimal pool build omitted virtio
serial. No assertion was removed. The new helper uses QEMU's upstream stdio
character backend over inherited connected descriptors and a separate console
file. Outer namespaces, closed input tree, dropped capabilities,
no-new-privileges and resource bounds remain. No extra seccomp layer claimed.

Build helper now enables upstream VIRTIO_PCI and VIRTIO_SERIAL in the support
configuration and verifies actual virtual-port realization in its smoke test.
Sanitized backend-build-328-pool PASS, SHA
2cebd1b0f94f98bd1bc1a4a9465dd9744f20201f4cadf8816ec7b2a79fe0812f,
19,570,240 bytes. Source archive stays pinned at 2a40e670...6cce. Exact generated
device configuration and source/tool inputs are in build-report/spec files.
No firmware backend or frozen demo binary was changed. Probe329 is ACTIVE71980.
Full current formatting, strict Clippy and workspace tests329 were started;
collect their session and logs. Formatting328 failed and was fixed with rustfmt.

Next: complete real worker transfer329, then test actual process execution and
SSH relay on this topology. Review nested namespace UID mapping before claiming
firmware execution: the native worker currently runs as private UID0; a further
namespace may map 0 to 0, which the firmware confinement check rejects. This is
an identified design question, not a tested failure yet. Preserve its isolation
contract. Current agent statuses remain quota-errored until25 October08:48;
independent review is still unavailable. Milestone2 remains incomplete.

05:25 UTC, checkpoint334. Full cycle322 COMPLETE PASS: all four boots, native
browser, signed update, settings restoration, restart persistence and eight
distinct Core acceptance/restart checks. Canonical assessment and driver-input
are retained. It explicitly leaves exact-board and full native-dashboard gates
incomplete. README and author review321 now point to this latest model result.

Worker transfer329 PASS in7.155 seconds: fresh1MiB upload/download, verified
destination hash1d65ea4a0d7b23f00cc5c6d1c50cbf6f627f99c72ce191c1f0cf3d3db971237c,
exact frozen263 worker identity and shutdown. Original source copy saved as
network-vm/native-worker-channel-329/source.rs. Full329 formatting, Clippy and
tests pass615 tests in105 binaries. These predate subsequent process work.

The execution probe now uploads a real closed process specification through
the same channel, obtains live status, retrieves/hash-checks the complete
result, checks the actual challenge digest and requires a cleanup receipt.
The implementation reuses coordinator transfer_object. Support module:
examples/support/worker_channel_execution.rs. Process330 FAILED before launch:
unshare denied the nested uid_map write. Its failed result was still retrieved
and checked, then cleanup acknowledged. The outer test timed out at180s because
Command retained the QEMU standard descriptors after spawn. Explicitly dropping
Command now lets worker IPC receive EOF if the VM fails before Shutdown.

Added optional ProcessSpec namespace_uid (default0, omitted on serialization).
The supported range is0..65534; only the invoking user/group are mapped. All
older constructors use0 and keep their wire representation. The new support
worker uses1000 and creates ordinary root-mapped private jobs. Both the worker
and VM have dropped capabilities and no-new-privileges. A real integration test
checks UID/GID1000, zero permitted/effective/bounding capabilities, no-new-privs,
one mapped identity, locked root, writable bounded scratch, and old wire format.
Initial331 test expected a global host UID in uid_map. Bubblewrap can insert an
intermediate namespace; the observed map was1000->0. The corrected test checks
actual credentials/capabilities and the single-entry map, without treating the
parent-relative number as a global UID. Root-guest confinement still rejects
0->0. Only the explicit non-root support check allows its intermediate mapping,
and additionally requires GID1000, no capabilities and no-new-privileges.

Process332 PASS with the actual frozen263 worker: isolated support process
executed outside the VM, hashed the fresh uploaded challenge, and its verified
result was retrieved and cleanup acknowledged. No SSH, firmware execution,
native dashboard, or physical-host claim. New host markers record UID/GID/map;
assessment now records main/module/confinement/runtime source hashes.

Full checks333 caught two compact ProcessSpec initializers missed by the first
mechanical edit. Both now select default0. Formatting, strict Clippy, workspace
tests and release helpers334 are ACTIVE; collect the session/logs. Then run a
fresh final channel probe, retain source copies, and record all source hashes.
Cleanup helper now also recognizes completed backend-build-NNN-pool paths;
the same locked, terminal-result and hash-verified deletion rules still apply.
Next clean backend328 and finished cycle322 duplicates. Do not delete original
evidence, source archives or protected canonical077 inputs.

05:45 UTC, after daemon restart. All earlier sessions are closed. Full334
formatting, strict Clippy and616 tests in105 binaries PASS. New current code
has later changes, so do not apply that check claim to it. Final component334
and frozen-helper repeat335 PASS. Frozen two helpers and checksum manifest:
channel-target-334/. Full source checkpoint334 is complete. README contains
the tested frozen-helper command. Author review native-worker-channel-334.md
records scope and open requirements. Cleanup334 removed2,632,440,758 bytes
from finished cycle322,250,773,936 bytes from its Core inputs, and698,349,720
bytes from backend328. All three restoration journals are retained.

New client-disconnect fault336 PASS as a negative test: exactly65536 bytes
acknowledged before injected client failure, no published object, no run,
both processes exit normally before the30-second guard. assessment.passed is
false and fault_gate_passed true; never count this as a successful transfer
or firmware test. The former Command-descriptor hang is no longer present.
The exact pre-format probe source is retained in its output/source.rs.

Preparation refactor337 moves bounded metadata capture, read-only source
object validation and LockedRun freezing from s9_execute_prepared into
examples/support/prepared_board.rs. Behavior is unchanged and all23 affected
example tests pass. This will be shared with the native-worker firmware path.
The full current checks still need to run after the implementation settles.

After restart, agents are available again. channel_review is independently
reviewing the worker/isolation changes. It found that console logs were read
only after VM exit and could be lost on an outer timeout. Parent now streams
the run-owned console file into bounded host trace while QEMU runs. Build338
was started; run the positive/disconnect tests and a real wall-limit regression
before calling the finding fixed. exact_target_gap checks one missing S9 input;
channel_artifacts owns ONLY support/worker_channel_artifacts.rs, a bounded gzip
codec for exact large worker results. Do not edit its file concurrently.

Next implementation: extend worker_channel_probe to execute the unchanged
prepared native-pcap-321 process through the native worker, using the existing
production WorkerExecutor for upload/status/logs/result/cleanup. Reuse the
shared prepared_board freeze function; do not duplicate it. The selected
process has53 mounts totaling205,103,977 bytes, including112,197,632-byte input.
Its exact limits are360s wall,330s CPU,3GiB address,64 processes,64MiB file/trace,
128MiB scratch. The outer support profile must explicitly allow a larger file
(at least128MiB), CPU budget and VM payload while keeping all inner settings
unchanged. Current outer110 CPU/64MiB file profile cannot run it.

The production result is about103MB, compresses to19.5MB. Let the actual VM
client retrieve and hash it via WorkerExecutor, then export a bounded compressed
copy. Host must check exact bytes before shared s9_execution_report retention
and frozen263 independent mining/Core checks. Avoid duplicating the large
artifact in both console markers and final observation. Preserve failed traces.
The topology still has no SSH, native dashboard or external physical-host pass.
Do not substitute a support-process pass for original firmware execution.

06:09 UTC, checkpoints341–344. New production WorkerExecutor path341 PASS
with exact worker-result bytes exported to the host, source hashes, live logs,
cleanup receipt and fresh payload. Disconnect341 remains a separate negative
PASS. Positive342 PASS with host identity checks. Stall342 PASS: real VM Probe,
then deliberate client stall,15-second outer wall-limit termination, actual
partial guest console retained, no invented success. This resolves the earlier
console-loss finding. Current topology still has no SSH or native dashboard.

Independent review is active again. channel_review fixed uncertain-response
reuse in owned DeviceTransport, ConnectedTransport and PrivateTransport. Both
socket transports now share one implementation; any failed exchange closes it.
Four real socket-pair tests include actual5-second timeout/late reply. Initial
peer tests failed because this host denies recv/send but permits File read/write;
corrected tests use File on the same real sockets. Integration recovery must
open a fresh channel after failed request; agent is fixing that test. Its review
is reviews/native-worker-independent-338.md. Current full checks are pending.

The artifact codec has10 passing tests and focused strict Clippy. It verifies
exact size/hash, one complete gzip member, no trailing bytes, bounded expansion,
private one-link source, inode/time stability, and atomic no-overwrite import.
The client now exports the actual received result before fallible later identity
or observation checks. The cleanup-to-host-export loss window remains OPEN:
client storage is ephemeral, and current production cleanup precedes host export.
IPC EOF also cancels active jobs. These cannot count as run survival/reconnect.

Firmware343 FAILED during client image preparation: result exit153, no trace
truncation,112,197,632-byte captured object exceeds64MiB file limit. Failed
preparation and all inputs remain in network-vm/native-worker-firmware-343.
Current fix344 gives isolated_network_vm an explicit128MiB per-file preparation
profile, with a255MiB summed base input bound and existing256MiB archive bound.
Other miner limits are unchanged. Build344 is active. Next run a fresh firmware
output, then shared report and frozen263 independent audits/two Core checks.

exact_target_gap found and reproduced a strict PCAP unsupported-destination
DMA hang on old320. New UBSan339 passes its134-transaction rejection regression
and normal118. Bad UNLOCK reset also fails on old320: after QMP reset CFG remains
zero instead of cold0x50b. Agent is fixing reset recovery and adding a device-only
migration test. Full S9 migration was already blocked by other unmigratable
board devices; do not claim a demonstrated full-board bypass. New model evidence
must stay distinct from historical native321/cycle322 model320 passes.

Exact evidence research is reviews/exact-target-gap-338.md, SHA
cbe50fa5a8025ddc871e4c5cd3ef4a4dd2d47ae09922463fa482e3b68ceaabf0.
Public zynq-io7f8e9a5 is G9/G19, not selected S9 logic. Empty S9 submodule points
to unavailable private x/braiins source; a short gitlink is not a pinned full
input. No repeated unchanged external download attempt. Exact board gates stay
open. milestones.md now removes the stale agent-unavailable claim and ambiguous
FPGA year. Available space is92GiB repository and14GiB/tmp. Protected canonical
/tmp/md-sim-qemu-backend-077-ubsan/inputs must not be deleted.

06:25 UTC, checkpoints345–353. Firmware344 preparation FAILED cleanly with
"compressed initramfs exceeds bounded export". Preparation345 uses a versioned
V2 export split at64MiB, total128MiB trace,128MiB file and the same256MiB expanded
archive bound. Its8 isolated_network_vm tests PASS, including independent cpio
and complete two-stream CRC/trailer checks. Two cleanup path tests also pass.

Native worker firmware345 COMPLETE process PASS in446,756ms. The unchanged
prepared native321 process executed outside the client VM. Host retrieved exact
worker result and shared execution reports. Frozen263 independent audits349
PASS:1,661 computed hashes,2 fresh jobs,clean-job transition,difficulty change,
11 matched/verified ASIC serial results. Two original Core checks350 both accept
and retain after restart (0->1->1), no node-generated block:
- bitcoin-core/channel345-candidate0-350:
  4cd4adfa036c6fe46f48e569a1fb71700d87cd0b877c471dd600ced20e63eb72
- bitcoin-core/channel345-candidate1-350:
  2e0de87b0274f4d9e8fb1e14cea8e020ea78565725bce0f1b9751d21a0f19066
These are adapted model320 execution identities. No exact board, SSH, dashboard
or physical host pass. This binary still precedes external retention barriers.

New runtime ProcessSpec.stdin_stream is default-omitted and permits only an
explicit protocol with1..65536 bytes, mutually exclusive with immutable stdin.
execute_with_streamed_input takes a supervisor byte channel and passes only a
pipe. Default execute rejects a declared stream without that handler. Actual
written bytes and errors enter ProcessResult.stdin_stream; default results keep
the earlier wire layout. Overflow cancels execution; timeout/exit closes the
namespace and ends blocked/idle input writers. Two real namespace tests347 PASS,
including exact binary bytes, closed legacy API, overflow, idle/unread timeout.
All existing constructors default to None. Current full checks are pending.

Agent added a shared production retention barrier to WorkerExecutor, with
run_with_retention/recover_with_retention. RetentionIdentity binds locked-run
hash,token,run ID and exact result ObjectId. Callback error or mismatch prevents
Cleanup. The external-required marker prevents accidental default-recover
bypass; recovery's cleaned fast paths also require the barrier. All17 library
and13 coordinator tests plus strict focused Clippy350 PASS. Real-worker test
withholds ack, changes each identity field, restarts worker on same root, opens
fresh channel, retains exact bytes, then verifies cleanup/recovery behavior.

New worker_channel_retention.rs provides outer-host exact import/fsync before
acknowledgment, over bounded recorded runtime stdin and a separate FIFO/virtio
port. The worker request stream is never multiplexed with receipts. Guest
exports exact result once before cleanup; final export adds receipt/cleanup
metadata. Positive component349 and351 PASS. Independent channel_review checked
actual349 sent stdin bytes against both receipts and exact result, and found no
new concrete isolation/retention defect. Active-run reconnect and outer-worker
root loss are still open; do not claim persistence from ephemeral worker storage.

Wrong-receipt352 correctly rejects the ack and keeps the worker's completed
result without cleanup. Its assessment FAILED because run_id was serialized as
OsString {Unix:bytes}. Fixed to checked UTF-8; fresh negative353 is running.
All failures remain. Review report: reviews/native-worker-independent-338.md.
Full Rust formatting waits for the PCAP agent's migration test edit to finish.

Cleanup352 reclaimed419,543,760 duplicate channel inputs with exact restoration
journal reports/duplicate-channel-input-cleanup-352.json. Canonical keeper is
network-vm/native-worker-channel-342/input-store. Preserve it. Agent separately
reclaimed2,095,049,160 duplicate finished backend339/341/343 inputs with journals.
New cleanup eligibility is restricted to numbered native-worker channel/run
stores and checks exact assessment/result/process hashes before locks/deletion.

06:36 UTC, checkpoint354/355. Wrong-receipt353 negative PASS after the UTF-8
run-ID fix. Worker retains its exact completed result, no cleanup or worker
retention receipt exists, and the guest records the mismatched outer receipt.
Its ordinary passed field remains false. Positive frozen-helper repeat355 PASS.
All659 Rust tests in105 binaries, strict workspace Clippy, release helpers,
format check and535 guide links PASS. Core-checkpoint-354 retains435 source
files plus the Rust library. Frozen helpers:
channel-target-354/release/examples/worker_channel_probe
  e8b50a959f454dbe7c7ffbff7b6bea4e1622988c3c4cf0deec1354f875f840d7
channel-target-354/release/examples/isolated_network_vm
  b3ebc0d0adc08cb430091f2261dc67031ee2e4051010da2fd716169508adb5b4
Their checksum manifest is retained. README now gives the tested355 command
and distinguishes pre-retention firmware345 from the new barrier path.

Fresh native-worker-firmware-354 is ACTIVE (tool session23016), selected
prepared native-pcap-321 unchanged, with full outer-host receipt barrier.
Current trace reaches captured-input import and locked transfer/execution.
After completion, run frozen263 regtest/rate audits and two Core checks. Do not
reuse345's mining result for this changed outer identity.

Parent reviewed agent-authored owned transport/common retention changes;
no unresolved concrete finding for their declared scope. Report:
reviews/coordinator-retention-parent-355.json. Independent agent reviewed the
parent's outer receipt/runtime path. Active-job reconnect and worker-root loss
remain open. All source changes after354 need affected checks again.

PCAP migration346 PASS on final UBSan343: actual generic outgoing+incoming
migration succeeds; strict outgoing/incoming fail at their respective hooks;
SW_ID/INT_STS unchanged after rejection;17 actual transactions; four clean
QEMU exits. Independent module review is reviews/pcap-migration-independent-355.md.
Exact-target agent prepares fresh original firmware on backend343 and will run
independent audits/two Core checks. No new browser cycle yet. It owns that run.
Known exact sources for preparation: automated-update-cycle-004/provision/
sd-snapshot-0.json SHA3ab9726f783cd3995c6708b78868acdc3d104dd44c31a020a01e2f6aa0fec8c6;
adjacent gzip SHA f4fb5a8b6980ac7f6e220f604bc1d6c981e08ed7ccc877d5a956a80c0fadd6b1;
console-startup-diagnostic-133.sh SHA
c7cbba85e2eb96bc3ebde8e3eb56901f930db3179b53a102246029bb8c50502e.

Channel-review agent now implements a small VM-side persistent owner-only
Unix-socket/DeviceTransport broker for actual SSH client loss. Reuse design:
reviews/native-worker-ssh-design-355.md. It owns new broker code and may expose
minimal shared bounded framing, but not worker_channel_probe. Parent will
integrate the SSH scenario after retained354. The broker preserves the native
channel across SSH session EOF; whole-VM EOF is a separate unfinished boundary.
Native dashboard needs a separate declared full-duplex run channel into the
pool-support VM and genuine 10.0.2.15:80 service path. It must not reuse a fixture
service or mix HTTP into worker JSON. RemoteGateway currently assumes worker
store files are local to SSH server; that must change for this topology.

06:51 UTC, checkpoints356–360. Retained native firmware354 COMPLETE PASS in
448,537ms. Exact worker-result103,542,771bytes SHA
0d8a5f376caca3cf1fdaeb4f09f85f96472be41a91f083c45c66c38f007f65b9.
Recorded stdin acknowledgment equals both host and worker receipts and this
exact result identity; cleanup follows the receipt. Frozen263 audits356 PASS:
1,322 hashes,14 independently matched serial results,two fresh jobs,clean-job
and difficulty changes. Both Core357 acceptance/restart checks PASS:
- channel354-candidate0-357:
  57d8deeb010c3f4ac1ed6af279e40dda63dfffedf70ff2815ba0ed9c0f06e723
- channel354-candidate1-357:
  380f9ca400eb8e5974ef7fe1c9f379d0fbfc87e8faec9d631271c728e77429e8
Combined report reports/native-worker-firmware-358.json; README has the actual
frozen354 firmware command. All these use adapted model320. No SSH/dashboard
or exact-target pass is claimed.

New final backend343 firmware349 also passes, owned by exact_target_gap:
1,326 hashes,11 verified serial results,two jobs,actual complete PCAP DMA,
two Core351 acceptance/restarts. reviews/pcap-native-firmware-351.md has exact
commands/pins. Prepared348 remains immutable/reusable. No native dashboard test.
PSU evidence research is reviews/s9-psu-identity-352.md. OEM manuals do not bind
a PSU revision to the selected firmware. Bitmain-hosted ACRT1901202 lists six
APW7 suffix variants, but raw PDF/label images are unavailable here. No exact
PSU pass, broad compatibility or incompatibility claim was made.

Cleanup helper now admits exact native-pcap-NNN and pcap/channel Core names.
Failed channel image preparations require real terminal failure metadata and
no started outer process. A new negative safety test refuses missing/successful
preparation or a later outer process. Three tests and focused Clippy360 PASS.
Preparation cleanup359 reclaimed1,409,159,578bytes from failed343/344, old345
staging and completed native349. Canonical keeper: native-pcap-321/input-store.
Journal reports/duplicate-channel-preparation-cleanup-359.json. Core cleanup359
first FAILED before deletion at its source-path guard. Fix360 permits only
completed declared native-worker-firmware-NNN/execution evidence. Cleanup360
then reclaimed992,864,866bytes; journal duplicate-channel-core-cleanup-360.json.
Canonical Core keeper: bitcoin-core/channel354-candidate0-357/input-store.
Do not remove either keeper. Original archives, source evidence, failed traces,
results and frozen helpers remain. Current free space89GiB repo,14GiB/tmp.

After checkpoint354, channel_review and its child changed worker framing:
CompletedWorkerError is typed only for a complete newline frame with matching
protocol,boolean false and a bounded valid error string. Owned transports may
reuse after that definite response; timeout/partial/malformed errors still
close permanently. Raw completed error envelopes are preserved by relay_request.
Child reports26 runtime,31 coordinator and5 real PTY tests plus focused Clippy
pass. These later changes need a new final workspace check and review.

Agent owns worker_device_broker.rs, support/worker_device_broker.rs and
support/worker_broker_regression.rs. Actual VM broker tests358/359 FAILED:
transfer/probe pass; in-VM debug worker times out on ExecuteToken. Broker closes
uncertain channel as required. Agent tries release worker with unchanged
protocol deadline;90-second startup budget was only diagnostic. No reconnect
pass yet. Parent must integrate actual SSH and native outside-VM path afterward.

New /root/private_channel agent owns isolation.rs plus a private_channel module
and real runtime tests. It implements a declared default-omitted Ethernet fd3
policy. Runtime creates socketpairs; nonblocking SyncSender<File> handoff gives
supervisor only a private endpoint. A bounded relay validates QEMU4-byte BE
Ethernet framing, bytes/packets and records hashes/EOF/errors. It supports the
separate streamed receipt stdin. No host FD/provider callback accepted, no
listener or host filesystem access. Host shutdown(SHUT_WR) is denied; carrier
EOF means whole-link disconnect. HTTP/TCP half-close remains ordinary frame
data, not carrier half-close. No dashboard claim from primitive tests.
Agent is mechanically adding default None constructor fields; do not conflict.

Next architecture: put independent pool in the permitted SSH-server/support VM,
with native miner QEMU outside. Pass one explicit private Ethernet link through
native job isolation. Reuse genuine miner NIC and pool path; expose native HTTP
only through the selected SSH tunnel/TLS endpoint. Keep pool identity, lifecycle,
limits, logs and cancellation in the locked run contract. This is a recorded
support adaptation requiring new mining/Core/dashboard tests. Do not silently
reuse frozen321 process identity or claim a physical remote host.

07:20 UTC, checkpoints361–372. Still milestone2 INCOMPLETE. User asked how
much remains; answered no reliable estimate, adapted mining passes while exact
board evidence and selected remote/dashboard gates remain open. Agents continue.
Broker361 release-worker VM regression PASS including real killed-client
recovery; reviews/worker-broker-361.md. This is a component in one VM, not SSH.

Parent added optional ssh-helper=REMOTE_NAMESPACE_CHECK to worker_channel_probe.
It stages a fresh 60-second support challenge process, prepares actual SSH
client/server roles via the helper, and runs the native worker outside the VM.
New support/worker_channel_ssh.rs restores the existing artifact codec export,
checks selected run, cancellation, advancing stdout, challenge and sequential
observations, exact result hash and cleanup. This first test deliberately uses
VM-local retention before cleanup; no outer receipt or whole-VM-loss claim.
Shared support/worker_virtual_port.rs validates exact virtio port name, safe
character file, O_NOFOLLOW, nonblocking open and matching sysfs major/minor.
Default channel and firmware modes remain separate.

Attempt370 FAILED at outer resource guard before guest: parent mistakenly
selected330 CPU seconds, above the existing120-second component ceiling.
Corrected to existing110 selected CPU seconds; no ceiling was weakened.
Attempt372 FAILED after real SSH transfer, ExecuteToken and live log retrieval:
`old client or SSH process survived client namespace termination`.
Frozen channel-target-372/release includes all five executable helpers with
SHA256SUMS. Native worker SHAfbae558c7872cbe372fb224fbedd8a26e6be3e3caf30e50bc0138a5c293dfa89.
Both attempts retain terminal result, assessment and logs. Agent channel_review
owns the SSH helper/broker and is investigating termination/reaping. It must
not mark wrapper exit alone as SSH loss. Parent owns worker_channel_probe,
worker_channel_execution and new worker_channel_ssh.

Private-channel agent reports real isolated carrier regressions365 PASS,
including an independent provider and SIGKILL supervisor-loss test killing all
four guest-side processes. Independent reviewer found host CPU spin on closed
source with blocked destination; fixed and measured at145,197us host CPU during
one-second wall limit (<400,000us assertion). Agent formats default constructor
fields and reruns checks. New runtime carrier remains separate from any native
worker/dashboard pass. No CLI private-channel binding exists yet.

Exact-target agent now researches minimal shared-pool integration. Initial
finding: disable both pool starters in s9_boot_bundle, inherit runtime-owned fd3
into miner QEMU and close the runner's copy after spawn. Support/SSH VM must
already run to issue Execute; its NIC therefore needs a run-owned socketpair
and bounded provider bridge when runtime offers the selected job endpoint.
Worker needs an explicit provider configuration tied to exact process identity;
no arbitrary caller FD can enter ProcessSpec. Keep pool seed/input/build/logs,
readiness, cancellation and resource policy in the locked execution contract.

07:44 UTC, checkpoints373–377. Actual SSH375 native outside-VM support process
PASS. Frozen helpers channel-target-375/release/SHA256SUMS. Native worker still
frozen372SHAfbae558c7872cbe372fb224fbedd8a26e6be3e3caf30e50bc0138a5c293dfa89.
remote_namespace_checkSHA79f9fb3a033a01a6fdfc77305d175e40abbe344af9420e0fef14b72ad198c248.
Exact canceled result7,535bytes SHA
be8d2a42cb705fdec9d6b69729dcdb246c7cdf480945f18041e7cd4955da5dfa.
Same run-1e17148f730c32fcedaf398d75bc803f0a1a90a88ac7784f18b33c472b7f4f81:
stdout391beforeloss→2399freshreconnect→3119later.23 challenge observations.
Two actual authenticated SSH sessions, oldclient/SSH processes gone, new
mount/PID/IPC/UTS namespaces, selected cancellation/result retrieval/cleanup
with exact acknowledgment. No firmware/dashboard/wholeVM-loss/outer receipt
claim. Independent parent-code/evidence review:
reviews/native-ssh-outer-review-375.md plus network-vm/native-ssh-channel-375/
outer-artifact-review.json. Reviewer authored guest namespace code, so that
part still needs an independent review. SSH373 FAILED a namespace inequality
after successful recovery: old namespace inodes had been reused.375 holds old
namespace FDs until comparison; unchanged inequality passes. New failure path
exports available exact artifacts before late checks.372 remained failed;373
records actual bwrap sleeping→zombie→reaped and SSH exit.

Private carrier+worker APIs now exist. Agent private_channel's53runtime tests
and strict Clippy pass; reviews/private-carrier-worker-independent-368.md has
no open delta finding. Worker binding consumes one exact canonical ProcessSpec
identity, rejects unconfigured/mismatch/reuse before run creation, preserves
idempotent token lookup and records consumed launch errors. Default no-channel
wire identity stays unchanged. Provider bridge reuses bounded framing/parser.

Parent new coordinator IPC path: src/bin/md-sim-worker-ipc/carrier.rs and binary.
Optional --private-carrier CONFIG_JSON SUPPORT_FD. Config pins full process,
support_identity must be captured input, handoff_seconds1..300, full relay
wall_seconds>=job wall. Both control/support FDs immediately CLOEXEC; same-fd
and same-inode aliases rejected. This also fixes potential old control-FD
inheritance; do not assert retrospective absence without test evidence.
Agent review found timeout treated as success, handoff consuming job budget,
and root lease/report overwrite races. All accepted. Parent fixed separate
handoff/full relay timers, failure on actual timed-out receipt, atomic
no-overwrite fsync report publication. Agent adds WorkerRootLease and owned
serve API so Provider holds root lock through final receipt. Parent377 tests
running, including real relay timeout and no-overwrite report. No CLI carrier
firmware pass yet. Parent owns IPC files, agent runtime files.

Shared pool implementation ownership:
- exact_target_gap: s9_boot_bundle.rs, support/s9_shared_pool.rs and minimal
  serial_console.rs after-spawn callback to close runner carrier immediately.
  Explicit shared-pool=MANIFEST mode, local-pool+console+prepare-only, mutually
  exclusive internal poolVM/browser/restarts. No synthetic success. Manifest
  fields in reviews/shared-pool-native-worker-366.md. read(path) returns strict
  (Manifest,bytes). Private process carrier descriptor3; observed miner window
  pool.seconds+5; actual pool success remains external evidence.
- parent: s9_pool_guest.rs and support/s9_shared_pool_service.rs, now adds
  --shared-server. Checks own verifier hash, exact eth0/lo/worker0 addresses,
  private direct routes, no v4/v6 forwarding/default route. Reuses existing
  independent pool server and privilege/resource drop. Focused376 test PASS;
  real new topology not yet run.
- channel_review: remote_namespace_check shared mode + selected readiness
  gate (target-specific wrapper around generic DeviceTransport), actual e1000
  into server namespace, client veth only, pool in separate /work sandbox.
  Starts after uploads, before matching ExecuteToken, waits actual readiness.
  Prepares full-firmware mode with declared large input/result limits.
  Owns new support/worker_shared_retention.rs types/constants/guest handshake.
  Planned joined receipt {native:RetentionIdentity,pool_bundle:ObjectId}.
  Guest combines exact native result + completed pool bundle; only outer
  durable acknowledgment permits common production cleanup. Parent will own
  host importer/outer execution in a separate module. Full support image hash
  belongs to outer run to avoid a recursive inner manifest identity.

Next parent actions: finish IPC review/test fixes; integrate optional prepared
shared target with SSH in worker_channel_probe, preserve inner ProcessSpec
private_channel only under exact shared manifest. Create native worker data
socketpair before support VM, pass trusted provider FD4 only to worker and
private Ethernet FD to support QEMU; runtime creates inner guest endpoint.
Capture manifest runner/backend/kernel/poolinput and complete support image.
Implement host joined-retention import with existing artifact codec and strict
result/pool identities. Join independent pool audits to exact native result
without changing result.json bytes. Then fresh firmware/ASIC audits/twoCore,
actual native dashboard selected run, SSH tunnel/TLS tests. Milestone2 still
open; exact FPGA/physical component/timing evidence remains unresolved.

08:07 UTC, checkpoints378–383. Parent shared path compiles(check381). CLI now
permits prepared=DIR plus ssh-helper=PATH only as declared shared firmware mode;
plain fixture/fault modes remain separate. New worker_shared_host.rs imports
exact native result through the common importer, exact PoolBundle via existing
gzip codec, checks lock/token/run/process/mounts/helper IDs, saves fixed pool
log/session paths and fsyncs joined receipt before ACK. New reusable receipt
scanner in worker_channel_retention.rs accepts marker/bound/host receiver;
old frame reader now reuses worker_retention_frame.rs. Protocol bounds: joined
request112MiB, pool bundle16MiB/compressed17MiB, ACK4096. Outer firmware trace
still128MiB, console120MiB; no resource ceiling silently raised.

Parent worker_channel_probe shared mode creates two separate private socket
pairs, gives native worker controlFD3/providerFD4 and VM EthernetFD3. Sources
are reserved>=64 before dup2; all other inherited descriptors are CLOEXEC.
VM retains control on stdio and receipts on separate virtio port. Only the
runtime-created endpoint enters miner job isolation. Provider config is a
captured input with full exact ProcessSpec/supportmanifest, handoff300s and
relay wall=jobwall+30. Parent validates actual helper/poolverifier/backend/kernel
against shared manifest before image preparation. Support-proof.json records
these plus exact scenario/initramfs/rootmanifest/outerprocess IDs before run;
outer-result.json adds exact parent result afterward. Final shared marker
MD_SIM_NATIVE_SHARED_POOL contains metadata-only native artifacts and exact
joined receipt, so host can compare worker cleanup/receipt after ACK. Actual
shared firmware run is still NOT executed; namespace helper implementation
is active under channel_review.

Agent private_channel independently reviewed all3 IPC wrapper fixes as resolved.
Actual CLI379 normal and canceled jobs PASS through native binary with real
isolated fresh challenge frames. Independent provider/runtime wire hashes,
result retrieval, cleanup and rootlease lifetime pass. Strict coordinator
Clippy and all coordinator380 targets including13queue tests PASS. Test helper
ELF capture factored under tests/support/elf.rs, no behavior shortcut. Parent
377four unit checks also PASS, including actual one-second relay timeout and
no-overwrite receipt. The native IPC binary is now CLOEXEC-protected for both
control/support descriptors. Full workspace checks still required after edits.

Default new code regression383 PASS with frozen channel-target-382 helpers:
- network-vm/native-worker-channel-383: passedtrue, exactouterretention.
- network-vm/native-worker-retention-383: ordinarypassedfalse, fault_gate_passed
  true, no worker cleanup/receipt after intentionallywrongACK. Expectedmissing
  worker-retention observation remains visible, not countednormalpass.
These prove normal/default transport survives shared-path/refactoring changes;
not firmware or sharedpool validation.

Cleanup383 validates new strict native-ssh-channel-NNN names,3testsPASS.
Reclaimed83,699,553bytes/147objects from370/372/373 client-image+execution stores
with canonical network-vm/native-ssh-channel-375/client-image/input-store.
PROTECT THAT NEW CANONICAL STORE. Journal
reports/duplicate-ssh-preparation-cleanup-383.json.
Reclaimed150,590,697bytes/60objects from370/372/373 outerstores with existing
canonical native-worker-channel-342/input-store. Journal
reports/duplicate-ssh-input-cleanup-383.json. No archive, evidence or executable
removed. Currentfree88GiB repo/14GiBtmp. README nowdocuments actual frozen375
SSH command, scope and independent report; links382540PASS.

Agent exact_target_gap owns external evidence validator s9_external_pool.rs,
prepared_board metadata, s9_execution_report and ASIC/submission audit shared
modes. It must not rewrite native result/logs or call historical354 split-pool
regression a new execution pass. Joined receipt/worker-retention exists only
AFTER ACK; preACK host importer does not call the reportretainer. Finalreport
runs aftermetadata arrives; independentminingaudit also requires outer-result.
Sharedrunner tests/12serialtests/lint/build366 PASS; actualshared execution
remains pending. Agent channel_review owns sharedVM topology/readinessgate and
guestpool/native joinedreceipt. Parent owns outer+importer and poolguestshared
networkguard. Agent private_channel owns CLIregression/reviews, no pending
runtime changes unless review requires them.

### 23 October 08:33 UTC — shared transport integration checkpoint388

Milestone2 remains incomplete. Default transport383 passes; wrong-receipt383
fails normally and passes only its explicit rejection gate. Real SSH375 passes
support-process transfer/reconnect/cancellation/cleanup. No shared native S9
execution has passed yet. Frozen matrix remains79,599 rows/zero full exact passes.

New shared support path is wired in Rust. It captures actual support helper,
pool verifier, QEMU/kernel, module, initramfs, root and outer process identities.
The native worker owns a separate private Ethernet relay for exactly the frozen
process. The server namespace starts the actual pool only for the selected
ExecuteToken and waits for its real readiness. The full native client retains
both exact worker result and separate pool bundle before joined outer ACK and
cleanup. Parent importer checks captured roots/mounts, provider receipt and
actual HOST build; pre-ACK import does not call the final firmware report.
Cancellation uses a private UID1000 file with exact cancel newline contents;
the pool joins session threads and records cancelled/incomplete work. IPv6
absence is explicit, not a successful inspection of nonexistent proc files.

Independent reviews of CLI carrier382, shared parent384, and external audit386
have no open source-scope findings. They do not grant an integration pass.
Affected external audit386 tests50 and probe48 pass. Coordinator380 tests36
pass, including13 queue tests. Actual native worker CLI379/382 complete and
cancel paths pass with fresh challenges, retained exact bytes and root leases.

Workspace check385 failed private PROTOCOL_VERSION and Spec equality. Runtime
now exposes its existing version constant; gate compares serialized policy.
Workspace check388 passes. Workspace strict Clippy388 is running. Duplicate
module inclusions are removed by common root aliases, without lint suppression.

Next: freeze helpers after focused review, prepare new shared S9 bundle using
backend343 at10H/s, run the full native worker/SSH/pool join. Audit both fresh
jobs and submit actual candidates to separate Core nodes. Retain all failures.
Then implement/test selected native dashboard SSH and TLS access. Exact FPGA,
component and physical timing evidence still blocks the exact-target gate.
Free space is88GiB on repository filesystem and14GiB in /tmp. Protected stores
listed above, including native-ssh-channel-375/client-image/input-store, remain.

Workspace test388 completed successfully:792 test executions across108 test
binaries;18 ignored child-helper entries remain explicit. Source was under
active review; this is not the final frozen release check. Independent review
then found two shared supervisor ownership issues: a root path-chown race in
pool cancellation, and broker-writable parent paths for root pool readiness
logs. channel_review is fixing both before the actual integration freeze.
Node helper now has explicit shared mode and bounded evidence copies; focused
tests390 and strict lint390 pass. Frozen helpers planned channel-target-390;
new preparation native-shared-prepared-390; integration native-worker-firmware-391.

Cleanup390 removed111,326,870 bytes of verified duplicate inputs from the
finished default/wrong-receipt383 runs. Journals: reports/duplicate-channel-
regression-cleanup-390.json (100,393,798bytes/40objects), and reports/duplicate-
channel-preparation-cleanup-390.json (10,933,072bytes/14objects). Canonical
342 and375 stores remain protected. Original files, logs and results remain.
The old release cleanup tool rejected the SSH canonical path before mutation;
the tested383 debug tool (c58a759f101fca27d415996b374331391257e72cecee71f28f65efa1af4ad6bb)
then completed that preparation cleanup. No unchanged external failure retried.

### 23 October 09:33 UTC — shared firmware391/Core392 pass; access implementation active

Milestone2/full task remain INCOMPLETE: exact FPGA, components, physical timing,
selected native dashboard SSH/TLS, clean installation/fullmatrix still open.
Latest user asks how much for milestone. Commentary says no defensible finish
estimate; ongoing test durations are evidence, not milestone ETA. Do not final
at this component success. User requested milestone review only after completed
milestone; milestone1 approved, milestone2 active.

Frozen390 completed: core-checkpoint-390 retains459sourcefiles plus builtRust
library, all source hashes checked after release build. Workspacefmt/strict
Clippy390 pass. First build invocation390 failed because s9_submission_audit
is a support module, not an example; corrected390b succeeds. Final binaries:
channel-target-390/release/examples/worker_channel_probe
  1415c810890dcd587aa5065dc9ce98b7382d99682bb8012eda7d49110e03f6ee
remote_namespace_check119874d949a81a2cede5dc64c37619631bd0dcac50d648b25dcb4fa4aa05d5dd
s9_boot_bundle1d003f457c7b115e2602484630a1799719054720ab3d270d3c4bab9a8a51cadd
s9_pool_guest39c7ff5c5d254e770963f6485cd7f28e114485c23363c2bf5c385fcf6d10a9fb
s9_asic_auditfa168a9ce9e944e0c012c1ac90b6baa8973ed7475d97045d0ea73f80734eaa6c
node_prerequisite_probe5565fdfaaa0784093d3bcd46e1affde216581ab143697b2d7a60f1662d96c863
workerIPC284c75db8e66430979bfb0eb1f7215e4902bb8e1fda406bbc63ec919ffbbdf2f
Full hashes channel-target-390/SHA256SUMS. Shared manifest same dir/shared-pool.json.

Prepared qemu-s9/native-shared-prepared-390 using backend343, exact same348
original SD inputs, budget360/pool300/10Hps/filter1/difficulty-31,-30, console133,
monitor031, savedSD004. Replaced pool-vm with shared-pool=frozen390manifest.
Command in checks-2026-10-21-001/shared-preparation-390.log and preparation
checkpoint. Native process file SHAfd0ecfc7f1c3f627b989000f1f0c1ef2dd52907efd6c776db18a73d336fdd3ad;
canonicalProcessSpec SHA182643d7234bbcd0b24c03dcf132b264bb83da3d97afbe866193a3e86f9077b6.
PROTECT this prepared inputstore; README/report repeat command uses it.

Actual network-vm/native-worker-firmware-391 PASSED outerexecution/retention.
Command: frozen390worker_channel_probe frozen390workerIPC backend328 OUT
prepared=native-shared-prepared-390 ssh-helper=frozen390remote_namespace_check.
Complete outer748209ms; native305143ms; no nativebudget/limits changed.
48objects/168074593bytes transferred before selectedpoollease at219.065s;
actualpoolready219.466s precedesExecute. Independentpool natural300408ms,10sessions.
Native105424186bytes SHA152ff586fe592184fb801a051fa89d0033a966f01cc0f763d24af84d754f08c2.
Poolbundle105365bytes SHA0c5dcd05f8bf9fad864e69b2419264a0f555548a292d669805d3e025f2410d4c.
Lockhash2e86c24b5655571b0e6a051c55f3f35f2c1fe302288908c21c06bc99cd8d3588.
Run run-50964620a12583b48e7af37dd2e9bb8842412fc2be9dc5d999453ac91b56c130.
Outerresult100153995bytes SHA080a11271e109129e2f5fa7e4c5e10bd362913f71d94158aae613366d8d7f0c8.
Provider/runtime agree124frames11159bytes guest→pool,92frames8367bytes reverse,
zero pending, samewirehashes. Native/host/joined/stdinACK receipts allmatch;
cleanup acknowledges exact native result onlyafterbothhostartifacts durable.
Independentreview reviews/shared-execution-independent-private-channel-391.md
plusmanifest verifies namespace/NIC/routes/mounts/receipts/rawpoolbytes.

Independent frozen390 audits392 PASS: execution/rate-audit392.json and
regtest-audit392.json;1351computedhashes,2freshjobs,cleantransition,difficultychange,
14serialresults,13matchedcomputations,monotonic10Hpscap. Selectedsession0.
BothactualCore392 nodesacceptandpersist0→1→1,notnodegenerated:
bitcoin-core/shared391-candidate0-392 block
04217364004c16bcc760830065147f2cc4adc7596be7a75424d68cc0270a356e;
candidate1 block24e3fdba0328dc7387b162eb916fa28d0ca679ef0e8de0646768ee53331de4d7.
ExplicitCLI node_prerequisite_probe DAEMON NEW_OUT shared REGTEST_AUDIT INDEX.
Actual commands/evidence recipe reviews/shared-core-handoff-390.md; initial
pendinglabel is historical, report shared-native-worker-394.md records actualpass.
README links report394; guide links394 pass542. New docs mean current source
no longer equals frozen390; frozen evidence stays intact.

POSTFREEZE393 followup: rootpoolService used blocking File::open for UID1000
broker lease/cancel paths. channel_review fixed O_NOFOLLOW|O_NONBLOCK owner/
mode/link/type/size checking inclbundlelease. Added15s bounded real diagnostics
fromactualbrokerrequest counts/files, no acceptance override.82helpertests+
strictClippy393 pass, independentreview shared-control-independent-private-
channel-393.md closed. Run391 does NOT test changedhelper; newintegrationrequired.

ACTIVE parallel source owners (do not conflict):
- channel_review: runtime worker.rs/worker_channel.rs and coordinator workerIPC
  carrier.rs/main + tests/worker_carrier.rs. Actualreadonly PrivateChannelStatus
  {token,challenge}, strict lowercase64hex; exactidentity/configuration, increasing
  sequence, actual provider waiting/active/closed/failed fromrelaythread withRAII.
  Optional config lease:{locked_run:fullLockedRun,policy:ObjectId}, acyclicoutside
  innerProcessSpec. Checks process equality/policycaptured. No-leaseoldpathsame.
  Nativeworkerstatus remains dashboard:null. Capability/readonlyretry added.
  CLI3985actualcasespass(normalleased,peerEOFwhilejobbooting,malformedfailure,
  pluslegacy/cancel), config/panic/runtime/retrychecks/lint pass. Reports provider-
  liveness-399.md +inputs. Agent awaitingindependentreview/sourcefreeze.
- private_channel: runtime/dashboard.rs extraction +newbyte_stream.rs/libexport.
  ByteBudget::new(connectionbytes,runbytes,idleseconds,cancel), relay(OwnedFd,
  OwnedFd,&ByteBudget)->StreamReceipt. Existingdashboard defaults/receipts kept.
  Owns captured runtime/access ignoredunit-test binaries in actualisolatedVM
  harness. VM398passedreal3MiBduplex/backpressure/halfclose/caps/cancel and
  accessTCP+Unix/lease/providerclose tests. Foundpostpollcancelfix; final399 VM
  rerun reportedlypass; getexactreport/identities. Reviews provider/access too.
- exact_target_gap: md-sim-access/src/carrier.rs,libexport,Cargo.toml/lockexisting
  sha2edge; now selected/remote/CLI explicitcarrier gateway branch. Corecarrier
  Policy/Selection/ProviderBinding/WorkerProviderTransport/CarrierService ready,
  independentreview closed, VM399pass. Existing runtime Binding path preserved.
  Important topology: VMserver cannotreadnativeworkerhostStore paths. Newremote
  gateway must retrieve boundedmetadata viaactualWorker Get overbroker socket
  intoownserverstorage, withhashchecks; neverassumesharedhostfilesystem.
- parent: S9policycapture and future guestaccess/browserwiring. New support/
  s9_carrier_dashboard.rs readsbounded same-open regularO_NOFOLLOW policy,
  requiresAdaptedNative,destination10.0.2.15:80,exactsharedmanifestidentity,
  policywall<=nativebudget. s9_boot_bundle CLI shared-dashboard=POLICY onlywith
  shared-pool; capturesbytesas /carrier-dashboard.json, recordsadaptation.
  Native360wall/300pool/305observation unchanged. prepared_board capturesexact
  validatedbytes, checksactualmount, addslock.policies.carrier_dashboard object.
  worker_channel_ssh writesoptionalprovider lease fullLockedRun+policy andchecks
  locked.process equality. ReviewfoundTOCTOU secondread; fixedexactbytes402.
  Tests400firstfailedmissing environment inunitfixture;401fixedall3examples
  passes.402affected2examples+strictClippy pass. Reviewcarrier-preparation-
  independent-400.md closed withfinalhashes. No actualnewdashboardrunyet.

NEXT parent work: implement supportserver owned carrier access service in new
remote_namespace_check/carrier_dashboard.rs, plusshared_pool preparation/role
mounts andworker_broker serverlifecycle, thenclient nativeHTTP/SSH/TLS/browser
probe andjoinedaccessartifacts. Coordinate withaccessagent. ProposedUID1000
service in ownbwrapmount/PID/IPC/UTS sharing servernetwork, mountonlyown/work,
ro /worker socketdir +capturedpolicy/providerconfig/helper/launchidentity. Parent
rootownslaunch/logfiles outsideunprivilegedwork, no pool/session visibility.
Start afteractualselectedLease.run_id, thenSelection+ProviderBinding+actual
SocketTransport/WorkerProviderTransport authorize. Ownhelperrefresh<=lease10s
andcloseondisconnect; realcontrolcallsbounded, notarbitraryactiveboolean.
CurrentgenericSSHtunnel andTLSGateway canreuse loopback +privateUnix endpoints.
OriginalworkerBinding cannotbefaked sinceProcessSpec.dashboard=None.
Neednew strictbrowserselection variant; isolatedbrowserprofiles/cookies.
Nativewindow may suffice withconcurrentobserver; do notsilentlyextend limits.
Fullnewtargetidentity/poolseed mustfreshmine/Coreagain afteraccesschanges.

Pinned QEMU328 sourcearchive2a40e6705e59fd8807012631bc6b2c0877f10481b12768f78e4f9d7b88376cce
verifiedread under protected/tmpbackend077store. net/socket.c EOF sets nc.link_down
butnopeerNICcallback; guestcarrierwasNOTproven livenessoracle. Actualproviderquery
is required; workerStatus alonecanremainbooting aftercleanproviderEOF.

Cleanup390 reclaimed111326870bytes fromfinished383 inputs withjournals; noarchive,
results orfrozenhelpersremoved. Free87GiBrepo/14GiBtmp at09:26. All canonicalstores
listedpreviouslyremainprotected. OnlyoneCargo targetcheck-target085. TMPDIRtest-tmp227.
Noactive391/Core392processesremain. Oldexecsessionsallfinished. AgentVMtests mayrun.

23 October10:09: see checkpoint-access-409.md for current access integration,
source owners, component checks and next actions. Native SSH/TLS/browser
integration is not yet exercised. Milestone2 and fullmatrixremainincomplete.

23 October10:28: frozen420 native-access-421 ACTIVE, execsession12324.
Source core-checkpoint-420 has473files; postbuildsha256checkALLPASS.
Fullworkspace412:853pass/0fail/23ignored across108suites. Ignored guest/VM
entries have their explicit prior component executions; no new integration
pass implied. Lastaffected417/418:158pass (90namespace+10browser+58probe),
workspaceClippy417PASS,finalfmtPASS. Independentreviews414,419plusgateway415
closed. NewguardedSSHchild regressionpasses, absoluteworkerdeadlineVM406PASS.
Helpers frozenchannel-target-420/release withSHA256SUMS. Sharedmanifest
04292e697ab226786af600bd0460b87beaa8cb79db8f2aee53cc455058e4f3f5/930B.
Carrierpolicy samefolder/carrier-dashboard.json:adaptednative10.0.2.15:80,
8connections,64MiB/connection,256MiB/run,360wall,10slease. No browserrequested
in421. Optionalssh-browser-payload nowexistsandisoriginalrequest/hashbound;
browsergate requiresfullsharedoracleandrawlog/processrecordjoins.

Prepared qemu-s9/native-access-prepared-420 using frozen420s9_boot_bundle:
boot original-zynq/sd-bundle-291 OUT spl sd2022-v1 backend=backend343ubsan
budget360 pool300 presencee0 fanfixture pic/asic/configurationcontracts
asic-packets local-pool shared-pool=frozen420manifest
shared-dashboard=frozen420policy temperaturefixture difficulty-31,-30
regtest-genesis cap10 fixedfilter1 batchconsole console-startup-diagnostic133
monitor031 savedSD004/provision/sd-snapshot-0.json prepare-only.
PreparedprocessfileSHAf3763cc20d311a02e72a793ee42c85e325f506d2e2e97cf9858fc35785738702/11222.
Allnative360wall/300pool/305observation/330CPUunchanged.

Actual421 command started10:26UTC:
channel-target-420/release/examples/worker_channel_probe
channel-target-420/release/md-sim-worker-ipc
qemu-s9/backend-build-328-pool network-vm/native-access-421
prepared=qemu-s9/native-access-prepared-420
ssh-helper=channel-target-420/release/examples/remote_namespace_check
(all paths prefixedwork/simulator). Logchecks-2026-10-21-001/carrier-native-421.log.
Liveguest.log markersMD_SIM_SHARED_POOL_PROGRESS; actualartifacttransferat75s,
no native run ID yet. Do notdumpgiantretentionlines; parsefixedprogressprefix.
SupportVM1536M; outer900wall/850CPU/4GiB limits unchanged. No otherCPU-heavy
buildsorVMs duringnativeexecution. Privateagentread-onlyactualidentityreview;
exactagentread-onlynext exact-target blockerstep;channel_review idleavailable.

When421finishes, inspectnative/access/poolactualresultsandfullreceiptbefore
claimingpass. Ifminingpathcomplete run frozen420s9_asic_audit rates-shared and
regtest-shared thenbothCorecandidatesviafrozen420node_prerequisite_probe shared.
Nativebrowserpayload024has916647483rawbytes/4548files; firstbasicrunomitsit
explicitly. Browserrunstillrequiredafternewaccesspathverified. Assesssupport
VMmemoryandretainactualresourcepolicy; neverextendnativebudgets silently.

23 October10:45:421FINISHEDFAILED. Actualbwrapchildstderr says--uidrequires
--unshare-useror--userns. Nochildservice/noHTTP;clientinspectiontimedout.
Native305149ms exit0 fulltrace;pool300512mscomplete10sessions; outer829296ms
QEMUexit0 butactualguestsuitefailed. DoNOTcountemulator exit0 aspass.
JoinedfailureevidenceretentionandactualstdinACK/cleanupPASSindependentreview:
reviews/failed-access-retention-independent-421.json. Nativec5efb13b...e247a/
105141709,pool7c9c0be6...cc251/107513,accessfed463a2...f693/448850.
Report reports/shared-access-failure-421.md. Session12324completedexit1.
PrivateagentfixingserviceUIDdropwithoutnewusermapping: bwrapownmount/PIDns,
minimalSETUID/SETGID/possiblySETPCAP, trustedRustentrydropsgroups/allUIDs/GIDs,
allcapsetsandNoNewPrivs beforemetadata/broker. Actualproductionlauncher
startupVMregressionrequiredbeforefreshfullrun. channel_review reviewsfix.
Noactualaccesspassandno421Corepass. Source420andfailed421remainunchanged.

Resourceagentreclaimed559650689B/325duplicateinputs viaexistingtesteddebug
utilityc58a759f;durablejournals/reviewresource-cleanup-421.md. Protectedstores,
archive/results/frozenhelpers/current420/421intact.399/406duplicates76072544B
keptbecauseoutsidewhitelist;noapprovalornewcleanupscopeassumed. Root~90.88GBfree.
Exactagent422readonlycomparesselectedFPGAtoarchivedcandidate:1964/5144frame
rangesdiffer. Fullcomparisonhashde57b4c03f05993a21c0cd22cbacb637d91ea7570399d57a49976796713eda23,
585280B;reviews/selected-fpga-frame-comparison-422.json. Publicmatchinghistory
notavailable,newqueriescachemiss/DNSfailure; reportmilestone2-next-dependency-422.md.
Noexacttargetgateclosed. Browserpayloadresourcebasis421recommendsbrowser-only
4096M VM/6GiBAS/explicitramfs; nativebudgetsunchanged. Parenthasnotimplemented
browserresourceprofileyet. Basiccurrently1536M/4GiB/900wall/850CPU.

23 October 11:08 UTC: startup fix426 and frozen build430 are complete.
The actual production launcher test426 passes. The service uses real UID/GID
1000, no supplementary groups, no capabilities, and NoNewPrivs. The launcher
starts at /, drops credentials, then enters /work/access. Failed421 and424
remain retained. See reviews/carrier-startup-426.md and the independent review.

Build430 contains 474 frozen source files. All captured source hashes match
the build checkpoint. Affected checks429 pass 90 namespace tests, ten browser
tests and 58 host probe tests. Formatting429 and workspace Clippy428 pass.
The full workspace tests412 preceded the last launcher/resource changes.

The browser-only support profile is implemented and reviewed, but not yet
executed: 4096 MiB RAM, two vCPUs, explicit ramfs, 6 GiB address-space limit,
1200 s outer wall limit and 2000 s outer CPU limit. Basic support and native
firmware limits are unchanged. See carrier-browser-resources-independent-430.
No browser pass is claimed from resource review.

Preparation430 completed with process specification SHA256
634a7c42faf9406a5f25990beba119aebef4893ff6367b051799de15d00239fb.
Native access431 is active under network-vm/native-access-431. It uses frozen
channel-target-430 helpers, native backend343 UBSan, support backend328,
native-access-prepared-430, and the production remote_namespace_check helper.
Output is saved in checks-2026-10-21-001/carrier-native-431.log; stdout is
empty during execution. At 180 s the actual broker was transferring inputs.
This first access run does not request a browser. Native limits remain 360 s
wall, 330 s CPU, 300 s pool and 305 s observation, with a 10 H/s cap.
Do not restart or overwrite431. Avoid builds and other VMs during its run.
The private_channel agent will review its actual results. The exact_target_gap
agent prepared one bounded archive extraction for after431. No exact target
gate has passed. The milestone estimate remains unknown.

Exact invocation, already running (do not execute again with this output):

```sh
work/simulator/channel-target-430/release/examples/worker_channel_probe \
  work/simulator/channel-target-430/release/md-sim-worker-ipc \
  work/simulator/qemu-s9/backend-build-328-pool \
  work/simulator/network-vm/native-access-431 \
  prepared=work/simulator/qemu-s9/native-access-prepared-430 \
  ssh-helper=work/simulator/channel-target-430/release/examples/remote_namespace_check
```

23 October 11:18 UTC: run431 is finished and FAILED overall. The root command
returned an error. QEMU exit zero is not a pass: the guest suite and assessment
failed at a single client TCP readiness attempt for the TLS listener. The native
HTTP suite did pass 15 requests through the actual SSH tunnel, including login,
original app asset, API data, configuration change/readback/restoration and
logout. No TLS request or browser test ran. See reports/shared-access-failure-431.md.

The native result ran 305154 ms, exit zero, untruncated; pool ran 300418 ms with
ten complete sessions; outer process ran 834098 ms. Retained native object is
e3aac175299649271bd985c54fd173541864d134c0c5f6bed46bf94776403cb1/105718948;
pool is 11be68bd9a2838456bab25fda1ccb945553d8bc3d9155b5ee9e9751bb490f52b/98537;
access is d8d56b446cbc8a9b368dce25237d9570dc716ed86f6f1773243dc88040e47b9b/5516831,
with 181 retained files. The actual joined ACK and worker cleanup are saved.
No new mining/Core pass is granted for431. No execution remains active.

Private_channel owns the minimal TLS readiness fix and an actual delayed-nginx
plus permanent-refusal regression. Use the existing absolute 30 s startup
deadline, retain exact connection errors, and check owned-child exit. Channel_review
will review this fix and the failed431 retention. Exact_target_gap may now run
its pinned archive extraction434 and existing isolated bitstream probe. No
archive program will execute. Parent owns reports, final checks, and the next
frozen full run. Preserve build430 and431 without changes.

23 October 11:35 UTC: TLS readiness source now waits within one existing
30 s startup deadline. It checks the owned SSH child before and after each
connection and retains actual socket errors and attempt times. Private_channel
owns this change in md-sim-access/src/remote_carrier.rs and its new test module.
Production source is unchanged since its captured435 version. Actual fixture435
failed on early child exit but did not retain the child error. Diagnostic437
retains the exact error: root nginx cannot find its default http account in
the minimal test VM. This is a fixture mismatch; production runs as UID1000.
Fixture438 now uses real UID/GID1000 and run-owned paths. Its 22 host tests pass;
the small VM is active. No readiness regression pass is claimed yet. Parent
must wait for its actual result and independent review before a full rerun.

Parent added bounded access diagnostics in shared_progress.rs. It emits file
metadata and a bounded passed/error summary from the actual client report.
It excludes response bodies and limits errors to 512 Unicode characters.
These are diagnostic observations, not gates. Namespace checks436 pass91,
zero failures, three ignored helper entries. Independent source review is clean.
Full workspace formatting, lint and tests remain required after the last fix.

Independent431 retention review is complete: every one of 181 access files,
ten pool sessions, two native copies and the actual joined ACK/cleanup agree.
See reviews/failed-access-retention-independent-431.md. Access remains failed.
README links reports/shared-access-failure-431.md. Guide check435 passes543 links.

Exact archive checks434/435 isolated the initial, recovery and nested update
FPGA payloads in the mixed2019/2020 installer. Two complete configurations were
found; neither matches legacy9250. The nested update is byte-identical to its
separately archived package. Original/recovery compressed bytes differ only
in gzip time fields, but exact expanded bytes match. The failed compressed
comparison is retained. No installer or firmware code was run for these checks.
See reviews/older-s9-archive-role-435.md and older-s9-executed-434.md. Verified
cleanup reclaimed 114601388 logical bytes of temporary input copies. Original
archive and derived evidence remain. A new public fork query did not resolve
the full e80262a source link. Exact_target_gap now reads the existing Bitaxe
core-mailbox continuation to identify useful independent implementation work.

The user again asked for milestone status and ETA. Milestone1 passed;
milestone2 is still one exact target plus remote workflow, with no reliable ETA
because exact FPGA/component/physical references remain unresolved. There are
three milestones; full matrix is milestone3. Do not redefine milestone2 as the
adapted SSH/TLS sub-path or stop after that intermediate success.

23 October 11:45 UTC: actual readiness VM438 PASSES. Seven real refused
connections precede listener success at 370 ms; authenticated TLS returns the
component body. Remaining-budget refusal ends at 30002 ms, an exited owned
child cannot pass via another listener, and Drop cleanup reaps the owned child.
Production source and final UID1000 fixture match the captured inputs.
Independent review carrier-tls-readiness-independent-438 is closed. This is
a component regression, not a native firmware TLS pass.

Final workspace440 checks pass: 863 tests, zero failures, 26 ignored entries,
108 suites; formatting, strict workspace Clippy and guide links pass.
core-checkpoint-440 retains 475 source files and the built static library.
All captured source hashes match after the release build. Frozen binaries are
in channel-target-440/release; SHA256SUMS verifies all 12 copies. Shared manifest
f620fabec3c7345c4d02e0b0a3f4d7bd80e2620f7ab1c625d5362e7b82aab702/930
selects helper f489c0669d00cad5f3eaec4e2e44881e5fcf784b7c0d2e5d95c590a5291593b9/6119664.
Native/backend/kernel hashes were re-read and match prior pinned343/328 inputs.
Preparation440 completed with ProcessSpec SHA256
8b532044ec994e74d17757fdeaa946fa819b656cd148f967c6432858200e6b67/11222.

Native access441 STARTED at 11:45:14 UTC. Active exec session is 93684.
Its full command is the431 command above with all430 replaced by440 and431
replaced by441. Output log: checks-2026-10-21-001/carrier-native-441.log.
Run directory: network-vm/native-access-441. No browser was requested. Native
360 s wall/330 s CPU, pool300 s, observation305 s and cap10 H/s are unchanged.
Do not restart or overwrite441. No builds, tests or other VMs during the run.
Channel_review monitors and reviews actual441 evidence. Parent runs fresh
mining/Core checks only after actual normal access/retention acceptance.

Private_channel is preparing a separate narrow cleanup utility extension for
completed byte-stream-NNN, worker-timeout-NNN and tls-readiness-NNN Stores.
It must check exact assessment/result/spec/log identities and terminal state,
keep exclusive locks and restore journals, and reject active or substituted
inputs. No build, test or deletion is authorized during441. Frozen440 is
unchanged; later cleanup source edits are not part of its tested source state.
The current plan retains102 candidates/114827208 bytes because the old utility
does not admit these paths. See reviews/tls-readiness-cleanup-plan-440.md.

Exact_target_gap continues bounded T17 readback static analysis only. Packing
for the original Bitaxe probe035 command is already implemented; write effects
remain unresolved. It identified software FOUND flags in the vendor collector;
do not model them as ASIC ready bits. No new exact-target pass exists.

23 October 12:27 UTC: T17 readback446 and packing447 are complete component
checks. The explicit --readback mode reuses the isolated t17_core_oracle path.
It executes the unchanged 916-byte collector and 168-byte unsigned-division
helper at their original addresses, with exact byte and RX-mapping checks.
All eight controlled queue cases pass (196 words). Wrong-chain and wrong-ASIC
cases confirm the static442 prediction: software FOUND becomes 1 while the
caller's initial deadbeef data remains. This is a driver result, not an ASIC
ready bit. No speculative clock-write or core-mailbox effect was implemented.

The actual compiler was Clang 23.1.1, with strict warnings and undefined-behavior
sanitizer traps on the new C wrapper. Original vendor bytes are unchanged and
are not instrumented. The original miner SHA-256 remains
ad090e7abbf525a7a8a23c7153529e45af94b864e1170e677e1f85b4c4091367.
The readback output is 784 bytes, SHA-256
2db7274bed8e9a8a90ac28af2c86b52613a7341c74357a6514a9417869d6b4bd.
Readback446 exited 0 in 138 ms and default packing447 exited 0 in 123 ms; both traces
are complete. Packing447 produced 49,152 words. Its output and ARM executable
are byte-identical to retained274. The old C, binary and JSON fixtures are
unchanged. Focused Rust tests pass 5/5, strict Clippy passes, release build passes
and focused formatting passes. These checks do not replace the next full
workspace check for the final combined source state.

Report and tested commands: reviews/t17-readback-executed-446.md. Exact source,
input and result pins: reviews/t17-readback-executed-446.sha256. Independent
review is closed with no unresolved finding in this oracle scope. It checked
all original function copies, executable mappings, output words, captured
result/tar/member bindings and the unchanged default path. See
reviews/t17-readback-source-independent-444.md and
reviews/t17-readback-execution-independent-446.json. README now documents the
explicit command, C ABI rationale and limits. No build or oracle remains active.
Original probe035 still stops at an unimplemented core-write effect; full
original-board and exact-target gates remain open. The duplicate-input plan
is reviews/t17-oracle-cleanup-plan-444.md; canonical274, unique objects and all
results remain intact. No oracle-store cleanup has occurred.

23 October: shared access441 and mining/Core443 are complete and pass in the
adapted reference scope. Report: reports/shared-access-pass-441.md. The exact
selected run is run-1c963695c1adf0903873ff423ff2f3c4be65537d03241ecd445b4fcef88c3268;
lock09e19cc31d326fbee7f9d88c8cbaeca636f390d0fc03106850b011fbeea4f5a9.
Both native and outer processes exited0 without truncation, in305154ms and
767202ms. Fifteen native HTTP exchanges through SSH and fifteen authenticated
TLS exchanges pass from the separate client namespace. No browser was requested.
The same exact native/pool/access results were retained before cleanup.
Independent review441 rehashed260 access files and all ten pool-session files.

Frozen440 audits443 verify1687 computations, seven matching submissions,
two fresh jobs, clean transition and difficulty change. Five shares passed;
two were LowDifficulty. Other negative-share cases are not established by
this session. Both isolated Core nodes accepted their actual submitted blocks
and loaded the same height1 tips after restart: candidate0
756bad500232f33432d1fb307b29420adf3c0177f6ba1b2443c3b3869b467a63;
candidate1 21c7823c03e5097c15eb5d705793371216fc3d4dc109ea3fc0679965d000d916.
See reviews/native-mining-core-links-independent-443.json and its input pins.
Milestone2 remains one complete exact target and remote workflow. Its acceptance
and unknown ETA are unchanged. No exact physical target or full-matrix gate passes.

Browser attempt445 is finished with exit1 before outer VM launch. Its builder
correctly pinned combined-initramfs.cpio.gz at499952513 bytes/SHA0765830df3072c8f270be2b43baf470b9075b502cf163d7ba7927a71d370b365.
The probe compared/imported the93609485-byte base image instead. The exact
failure remains at reviews/native-browser-preparation-failure-445.md and
network-vm/native-browser-445. Parent owns the shared selected-image fix for
launcher, support proof and independent audit, including payload provenance.
Keep native budgets unchanged. No Store quota increase is needed:512MiB is
per object and total quota is16GiB. A new reviewed/frozen browser run is next;
445 supplies no browser or guest-execution pass.

Cleanup444 passed nine ordinary tests, the separately executed actual CLI
scratch test, strict focused Clippy and formatting. Independent source and
mutation reviews are closed. Only94 exact duplicate CAS objects were removed
from finished399/406/438, reclaiming114348280 bytes. All remaining files and
all restore copies match the before-state. Failed435/437 are ineligible and
unchanged. See reviews/component-cleanup-444.md, its durable restore journals
and postcheck.json. Original archives, unique helpers, frozen440 and441 were
not deleted. The cleanup source delta is separate from frozen440.

23 October, selector fix449 in progress: parent added shared
examples/support/prepared_vm_image.rs and wired worker_channel_probe,
worker_channel_ssh support_proof and s9_external_pool to it. Browser requests
select the combined image; basic requests keep the base image. All prepared
image/kernel/root hashes stay mandatory. The launcher imports the exact
validated ObjectId. Payload checks bind the original request, copied manifest,
payload report and final image. Result audits use retained bytes and do not
open the original source host path. The launcher checks that source before
preparation. The independent reviewer found and parent fixed one misplaced
variable capture. A preliminary release build passes but emitted one unused
field warning; parent now uses that selected object explicitly in the audit.
This final source delta is not yet rebuilt or tested.

Exact_target_gap owns only prepared_vm_image_tests.rs, with tiny real
concatenated gzip/newc fixtures, Store readback, identity/substitution negatives
and read-only retained445 replay. Parent owns all builds/tests. Channel_review
reviews final source and evidence. Planned IDs: selector449 checks, final450
checks/source freeze/helper copies, browser451. No VM is active. Do not reuse
failed445 as execution evidence, overwrite frozen440, or alter prior441/443
results. Current free root storage is about78GiB; this is not a storage blocker.
After tests and review, prepare and run451 with frozen450 helpers, then perform
fresh mining/Core checks on its actual outputs. Continue milestone2 work;
79,599 frozen matrix rows still have zero full exact-target passes.

23 October 12:49 UTC: selector fix and full450 source are frozen and reviewed.
Full workspace450 passes908 tests,0 failures,33 ignored entries,108 suites;
formatting, strict Clippy and545 guide links pass. The six selector tests
exercise real gzip/newc bytes, exact Store import and an independently decoded
second archive. The explicit retained445 test first failed because its relative
environment path resolved from Cargo's crate directory. That failure is saved.
The unchanged test passes with the absolute run path and verifies combined
0765830df3072c8f270be2b43baf470b9075b502cf163d7ba7927a71d370b365/499952513.
See reviews/browser-selector-independent-450.md and its exact input manifest.
No VM or firmware pass is inferred from that retained-preparation check.

core-checkpoint-450 retains480 source files plus the built model library.
All480 source hashes match after release builds. channel-target-450/SHA256SUMS
checks all12 frozen executable copies. Shared manifest is
88509d6828c9bb1323b235919cc0b2d030245e1ca2b0666d83b3c4b0526225f1/930;
support helper67b7d1c901bddac979e23fea00dc562e2fc2358dde02b53cb551e9b2a2465c04/6150128.
Prepared native process in qemu-s9/native-browser-prepared-450 is
b5e012ccb1dda045c4e8e7bf89d83e95bc854679c27e0726f78eed7d1f9c86ae/11222.
Native360s wall/330s CPU/3GiB, pool300s, observation305s and cap10H/s are unchanged.

A new450 rate audit of historical441 passes base-image selection and retained
input checks. Its regtest audit rejects the old retained join: the verifier
source identity changed and the new image-verifier identity is now recorded.
All other join fields match (apart from the expected later outer-result field).
See reviews/selector-basic-audit-version-450.json. Keep this rejection; do not
weaken the strict equality or overwrite prior443 evidence. Fresh451 uses450
for capture and audit, so it must produce its own fresh mining/Core evidence.

Browser451 STARTED. Full command:
work/simulator/channel-target-450/release/examples/worker_channel_probe work/simulator/channel-target-450/release/md-sim-worker-ipc work/simulator/qemu-s9/backend-build-328-pool work/simulator/network-vm/native-browser-451 prepared=work/simulator/qemu-s9/native-browser-prepared-450 ssh-helper=work/simulator/channel-target-450/release/examples/remote_namespace_check ssh-browser-payload=work/simulator/network-vm/native-driver-024/payload.json
Log: checks-2026-10-21-001/carrier-browser-451.log.
Do not overwrite451. No builds/tests/other VMs during this actual run.
The explicit browser support resource profile remains unchanged. User timing
questions are steering the task, not cancelling it. Exact_target_gap is reading
selected SPL clock/poll/delay tables without execution; no calibration claim.
Active browser451 exec session:99733. All earlier build/audit sessions ended.

23 October 12:55 UTC: browser451 FAILED before worker/native/browser startup.
Session99733 ended exit1. Correct combined image9fad1790adc558699bd7b9075d378713d189c638859e301f0d14742a55ebae82/499993033
booted the support kernel and actual Rust PID1. The first guest scenario
error is 'component input count exceeds bound', then init exits and the
kernel panics. Outer process exits0 after44051ms, untruncated, but the probe
correctly records assessment.passed=false and no native carrier receipt.
Emulator/kernel start is not firmware or browser success.

Root cause: remote_namespace_check::initialize_identity captures /usr with
browser=true (entry cap8192), then uses capture_inputs(...false) for shared
pool and carrier files. That resets the aggregate cap to1024 after the
browser inventory already exceeds it. Finalrootmanifest has4632 regular
mounts. Private_channel owns a minimal fix with an inventory-wide entry
limit and unchanged per-path strict shared-file link/size rules, plus
actual-file bound regressions. Channel_review independently reviews451 and
that fix. Builds/tests are allowed again; no VM is active. Frozen450 and
failed451 stay unchanged. Next reserved freeze452/browser453.

Exact_target_gap now implements only new inspect_s9_spl.rs and support module
from the independently reviewed firmware-timing-recovery-450.md. It will pin
exact boot.bin/SPL suffix/code ranges, all revision table branches, operation
bounds, any-bit polling and wrapping timer ticks. No new CPU emulator, runtime
clock model or calibration claim. No test/run until coordinated; use454 for
that component. User questions about reverse engineering remain task steering.

Inventory452 fix is source-final. One entry_limit now applies to the entire
inventory (basic1024/browser8192). Per-path symlink and256/512MiB size rules
stay separate. Revisits and empty directories consume no new slot; the next
unique record fails before mutation. The actual451 initial inventory was4619
regular files plus1105 links, or5724 records. Changed sources are only
remote_namespace_check.rs and new input_inventory_tests.rs. Checks pass101
ordinary helper tests and6 explicit ignored entries; a separate isolated
regression passes1/0ignored. Its actual child exits0/untruncated in180ms,
with the host sentinel absent. Strict Clippy and scopedformat pass. The
independent review is closed with no finding. See reviews/input-inventory-fix-452.md
and reviews/input-inventory-independent-452.md with exact input manifests.
Frozen450 and failed445/451 remain unchanged. No VM is active.

Exact_target_gap now owns Cargo for SPL454 focused tests, explicit retained
input replay, strictClippy, release build and isolated static inspection.
Source is new inspect_s9_spl.rs and support/s9_spl_contract.rs only. Output
will be original-zynq/spl-contract-454. Its source review is clean. The agent
must fix and retain any check failures, complete actual inspection, and add
the tested README command. Parent then runs fullworkspace452 checks, freezes
new452 helpers and support/native manifests, and starts freshbrowser453.
Freshmining/Core checks require453's own actual results. The static inspector
does not execute original ARM code or establish FPGA/physical calibration.

23 October 13:19:50 UTC: browser453 STARTED, active exec session59665.
Full452 checks pass919 tests,0 failures,36 explicit ignored entries,109 suites;
formatting, strict workspaceClippy and546 guide links pass. SPL454 completed
actual isolated static inspection:654 records across15 tables agree with the
independent review. Its eight focused checks (including explicit retained
input test), Clippy/build/format and report hashes pass. README has the tested
command. See reviews/s9-spl-extractor-executed-454.md and
reviews/spl-inspector-independent-454.md. This is static evidence only.

core-checkpoint-452 retains483 source files plus the built model library.
All483 source hashes match afterrelease. All12 frozen binaries verify under
channel-target-452/SHA256SUMS. Only remote_namespace_check changed from450:
dbe0016baac184630131db94c702d70ce4cde6a370fc26e120a3803f23659c10/6151968.
Shared manifest d7a6f68d6df62be8f9efa682de3686b559a477076c48943c18e21dd1dfa17e2c/930;
carrierpolicy c144a8dc3ec790f670118df037ac4a0b954e6aed50fc3a5af1a9602302f5ae70.
Native prepared ProcessSpec efd259f7b144af74a71c89b2886a36af0a5ce5924cebc9acd714f9883cf505cf/11222.
Native360s wall/330s CPU/3GiB, pool300s, observation305s and cap10H/s remain.

Exact453 command:
work/simulator/channel-target-452/release/examples/worker_channel_probe work/simulator/channel-target-452/release/md-sim-worker-ipc work/simulator/qemu-s9/backend-build-328-pool work/simulator/network-vm/native-browser-453 prepared=work/simulator/qemu-s9/native-browser-prepared-452 ssh-helper=work/simulator/channel-target-452/release/examples/remote_namespace_check ssh-browser-payload=work/simulator/network-vm/native-driver-024/payload.json
Output log: checks-2026-10-21-001/carrier-browser-453.log.
Do not overwrite453 or start builds/tests/otherVMs during this run. Current
root free space about73GiB and/tmp11GiB. No storage blocker. Private_channel is
staging a narrow T17 cleanup extension under work/simulator/cleanup-development-455,
not editing production source or deleting anything. Canonical274/original
archives and all445/451 failures stayprotected. No physical/exact-target gate
has closed. Milestone2 and fullmatrix remain incomplete.

23 October 13:38 UTC: browser453 endedFAILED, session59665 exit1. The user
interrupted one wait, but the owned run continued to completion. Outer process
exited0/untruncated after759518ms; assessment.passed=false. Native result is
exited0/untruncated after305140ms. Both actual HTTP and TLS reports contain15
passing native exchanges. Firefox profile0 failed NewSession before any
browser report. Its300s startup deadline expired while waiting for the initial
application window. SWGL framebuffer warning also occurs in passing284/322,
so do not call that warning the cause. Raw diagnostics show2.1GiB MemAvailable
and a running Firefox main thread, not a demonstrated memory-cap failure.

Selected453 run is run-d7b13b55416780e0443c175955b8ec9dc008b888a58bcb8bedd330e6b5315cc4.
Combined supportimage04a8ffa153b70d2fe888fcf83e263614a1c93d698df77fb45d514e11d58a329a/499985342;
finalrootmanifest6535b6e319aa983720a4ea4dafd5c5f37d1228ed0511c6447a145d53e9f1bd95/874638.
49 transferred objects were destination-verified. Full native/pool/access bytes
and joined receipts are retained; independent exact-retention review continues.
OuterresultSHA3fcb690196c2d3ee6d2ae82ff0c5cba11625a166fd7d547a46913e197f072c92/210923341.
The access client also reports missing gateway-stop acknowledgment. Server
records say it stopped the selected gateway; tunnel stop passed. Keep the
client acknowledgment failure explicit, not a clean lifecycle pass.

Channel_review reconstructed the exact86428-byte start diagnostics from matching
overlapping retained prefix/suffix. See reviews/browser-start-failure-453/
(start-failure.json,0-stdout.log,0-stderr.log,provenance.json). No original
artifact was changed. Native/pool mining has not yet received new audits/Core.
The strict regtest tool needs pool-result.json, which failedclient453 did not
create. Do not insert a fake pass or weaken original audit assertions.

Private_channel found passing284/322 support used4vCPUs and256MiB TCG translation
cache.453 used2vCPUs and64MiB; exact Firefox/libxul/omni/fonts/runtime bytes match.
It now owns minimal --selected-startup browser regression source and Cargo/VM
coordination: sameproductionprofile, UID1001/cgroups/cert verification, twofresh
profiles and about:blank/cookie/cleanup checks; firmware/dashboardgates false.
Use pinned328backend and onefrozenfixture:456build,457baseline2CPU64,
458variant2CPU256; onlythenmeasure4CPU256 ifneeded. All300/270s startup limits,
RAM andisolation stayunchanged. No productioncachechangebeforemeasurement/review.

Cleanup455 remains staged inwork/simulator/cleanup-development-455 only, reviewed
but unbuilt/unexecuted; no deletion/integration. Againstprotected274, declaration
matches total1623481881bytes across272/446/447, pendinglocked stored-byte checks.
Source ofactive cleanup utility is unchanged. Exact_target_gap now stages a
minimal shared-mining recovery audit underwork/simulator/shared-recovery-development-459,
reusing strictexistingjoin/Bitcoinverification, withseparateoutput andaccessfailure
preserved. No productionedits/builds/VM untilcoordinated. No otherVM isactive yet.

## 23 October 2026: milestone 1.2 stop and handoff

Latest user instruction: add reachable milestone1.2 before2, stop to save tokens,
and leave Git setup and the commit to the user. Milestone1.2 passed for adapted
run441 with independent mining/Core443 and selected SSH/TLS API checks. See
`milestone-1.2-handoff.md` before resuming. Milestone2/full task remain incomplete.
No simulator VM remains active. Do not run458 or integrate455/459 until resume.

Final current-source formatting and strict workspace Clippy pass. Workspace
checks:919 passed,0 failed,38 explicitly ignored,109 suites. Logs retained in
`milestone-1.2-checkpoint/`. Ignored helpers are not execution passes. Baseline457b
finished before stop: two real selected-client Firefox startups202007ms and
202503ms, outer467881ms/exit0/no truncation. Browser453 and gateway acknowledgment
remain failed. No production cache/deadline change and no458 test occurred.
Recovery459 and cleanup455 are staged, unbuilt and unintegrated. Recovery459 has
an unresolved whole-file clone bound under concurrent source growth.

The root.git is empty and read-only. Earlier git init failed. The user will
handle it; no agent commit or alternative snapshot repository was made. New
.gitignore preserves source/vendor/catalogue records/compact evidence/staged
work and excludes payloads, raw runs, build caches and credentials. Its33
positive/negative matching cases pass using real Git in a removed temporary
repository. Ignored data remains in place; Git is not the archive backup.
